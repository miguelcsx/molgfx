//! Allocation-stable clipmap selection over sparse catalog metadata.

use super::{
    BrickSelection, BrickSelectionScratch, BrickWorkingSet, BrickWorkingSetError, ClipmapLevel,
    ClipmapQueryStats,
};
use core::ops::ControlFlow;
use molgfx_core::BrickCatalog;

/// Immutable clipmap policy; traversal writes into caller-owned scratch.
#[derive(Clone, Debug)]
pub struct ClipmapSelector {
    levels: Box<[ClipmapLevel]>,
}

impl ClipmapSelector {
    /// Validates unique mip levels and nondecreasing ring radii.
    ///
    /// # Errors
    ///
    /// Returns [`BrickWorkingSetError::InvalidClipmap`] for an empty or
    /// inconsistent policy.
    pub fn new(mut levels: Vec<ClipmapLevel>) -> Result<Self, BrickWorkingSetError> {
        levels.sort_unstable_by_key(|level| level.mip);
        if levels.is_empty()
            || levels
                .windows(2)
                .any(|pair| pair[0].mip == pair[1].mip || pair[0].radius > pair[1].radius)
        {
            return Err(BrickWorkingSetError::InvalidClipmap);
        }
        Ok(Self {
            levels: levels.into_boxed_slice(),
        })
    }

    /// Selects sparse descriptors intersecting configured rings.
    ///
    /// Runtime cost depends on catalog metadata and resident pages, never on
    /// logical voxel count. The method performs no allocation when `scratch`
    /// has sufficient fixed capacity.
    ///
    /// # Errors
    ///
    /// Returns typed backpressure when selection scratch is full.
    pub fn select_into(
        &self,
        catalog: &BrickCatalog,
        working_set: &BrickWorkingSet,
        focus: [u64; 3],
        scratch: &mut BrickSelectionScratch,
    ) -> Result<ClipmapQueryStats, BrickWorkingSetError> {
        scratch.clear();
        let mut total = ClipmapQueryStats::default();
        for level in &self.levels {
            let mut capacity_error = None;
            let visited_nodes =
                catalog.query_mip(level.mip, focus, level.radius, |descriptor, distance| {
                    total.visited_descriptors += 1;
                    let Some(distance) = distance else {
                        return ControlFlow::Continue(());
                    };
                    let selection = BrickSelection {
                        descriptor,
                        resident_slot: working_set
                            .get(descriptor.metadata.id)
                            .map(|page| page.slot),
                        distance,
                    };
                    match scratch.push(selection) {
                        Ok(()) => ControlFlow::Continue(()),
                        Err(error) => {
                            capacity_error = Some(error);
                            ControlFlow::Break(())
                        }
                    }
                });
            if let Some(error) = capacity_error {
                return Err(error);
            }
            total.visited_nodes += visited_nodes;
        }
        total.matched_descriptors = u32::try_from(scratch.entries().len()).map_err(|_| {
            BrickWorkingSetError::CapacityExceeded {
                resource: "u32 clipmap metrics",
                capacity: scratch.entries().len(),
            }
        })?;
        scratch.sort();
        Ok(total)
    }
}
