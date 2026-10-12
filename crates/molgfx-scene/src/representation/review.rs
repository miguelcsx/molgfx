//! Cold, immutable target reviews retain shared storage and a sampled pose.
//! Capturing costs O(1); distance geometry builds one hierarchy on the worker.

use super::{RepresentationApplicability, form::RepresentationFormSpec};
use crate::{Error, RepresentationSpec};
use molgfx_core::{StructureAsset, TrajectorySegment};
use std::sync::OnceLock;

#[cfg(test)]
#[path = "review_tests.rs"]
mod tests;

/// A molecular target captured for geometry review outside the render thread.
/// Atom coordinates remain shared; later playback cannot change this review's pose.
#[derive(Debug)]
pub struct RepresentationReview {
    asset: StructureAsset,
    trajectory: Option<TrajectorySegment>,
    prepared: OnceLock<Result<molgfx_core::Scene, String>>,
}

impl RepresentationReview {
    pub(crate) fn new(placed: &molgfx_core::PlacedStructure) -> Self {
        Self {
            asset: placed.asset().clone(),
            trajectory: placed.trajectory().cloned(),
            prepared: OnceLock::new(),
        }
    }

    /// Tests the exact form and selection at the captured presentation time.
    ///
    /// # Errors
    /// Rejects invalid queries, controls, unsupported topology or exhausted geometry budgets.
    pub fn applicability(
        &self,
        specification: &RepresentationSpec,
    ) -> Result<RepresentationApplicability, Error> {
        if !matches!(
            specification.form,
            RepresentationFormSpec::DynamicBonds { .. } | RepresentationFormSpec::Polyhedra { .. }
        ) {
            return specification.applicability(self.asset.source());
        }
        let prepared = self.prepared.get_or_init(|| {
            let mut scene = molgfx_core::Scene::new();
            let handle = scene.add_asset(&self.asset);
            if let Some(segment) = &self.trajectory {
                scene
                    .set_trajectory_segment(handle, segment.clone())
                    .map_err(|error| error.to_string())?;
            }
            Ok(scene)
        });
        let scene = prepared
            .as_ref()
            .map_err(|message| Error::InvalidSpec(message.clone()))?;
        let (_, placed) = scene.structures().next().ok_or(Error::MissingId)?;
        specification.applicability_at(placed)
    }
}
