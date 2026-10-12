//! Resident intervals are validated against a proposed resolution before publication.
use super::{Resolution, Scene, overlay::PlaybackPair};
use crate::{Error, StructureId};
use std::collections::BTreeMap;

pub(super) struct PreparedResolution {
    pub(super) resolution: Resolution,
    pub(super) playback: BTreeMap<StructureId, PlaybackPair>,
}

impl Scene {
    pub(super) fn prepare_resolution_playback(
        &self,
        mut resolution: Resolution,
        sources: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    ) -> Result<PreparedResolution, Error> {
        let handles = sources
            .keys()
            .copied()
            .zip(resolution.scene.structures().map(|(handle, _)| handle))
            .collect::<BTreeMap<_, _>>();
        let mut playback = BTreeMap::new();
        for (&structure, pair) in &self.playback {
            let Some(source) = sources.get(&structure) else {
                continue;
            };
            if pair.source != (source.identity(), source.coordinate_revision()) {
                continue;
            }
            let handle = handles.get(&structure).copied().ok_or_else(|| {
                Error::InvalidSpec(
                    "playback placement is absent from the proposed resolution".to_owned(),
                )
            })?;
            let segment = molgfx_core::TrajectorySegment::new(
                pair.start.native()?,
                pair.end.native()?,
                pair.sample_seconds,
            )?;
            resolution.scene.set_trajectory_segment(handle, segment)?;
            playback.insert(structure, pair.clone());
        }
        Ok(PreparedResolution {
            resolution,
            playback,
        })
    }
}
