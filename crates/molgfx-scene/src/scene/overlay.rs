//! Runtime overlay bindings and renderer-handle inspection.

use super::{Resolution, Scene};
use crate::error::Error;
use crate::id::StructureId;
use crate::overlay::{OverlayHandles, TrajectoryBinding, TrajectoryFrame, VolumeBinding};

impl Scene {
    /// Declared overlays whose runtime bulk source is not yet bound.
    #[must_use]
    pub fn unresolved_overlays(&self) -> Vec<crate::OverlayId> {
        self.spec
            .volumes
            .keys()
            .filter(|id| {
                !self
                    .overlay
                    .volumes
                    .iter()
                    .any(|(resolved, _)| resolved == *id)
            })
            .map(|id| crate::OverlayId::Volume(*id))
            .chain(
                self.spec
                    .segmentations
                    .keys()
                    .filter(|id| {
                        !self
                            .overlay
                            .segmentations
                            .iter()
                            .any(|(resolved, _)| resolved == *id)
                    })
                    .map(|id| crate::OverlayId::Segmentation(*id)),
            )
            .chain(
                self.spec
                    .trajectories
                    .keys()
                    .filter(|id| {
                        !self
                            .overlay
                            .trajectories
                            .iter()
                            .any(|(resolved, _)| resolved == *id)
                    })
                    .map(|id| crate::OverlayId::Trajectory(*id)),
            )
            .collect()
    }
    /// Binds one immutable density grid for every volume descriptor naming it.
    ///
    /// The descriptor stays portable: a volume whose source has no matching
    /// binding resolves to no handle, which is the intended contract rather
    /// than an error, because the same specification may be resolved on a host
    /// that holds different bulk data.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for a malformed grid, a content
    /// hash that is already bound, or a grid that contradicts a descriptor it
    /// would satisfy.
    pub fn bind_volume(&mut self, binding: VolumeBinding) -> Result<(), Error> {
        let mut bindings = self.overlay_bindings.clone();
        bindings.insert(binding)?;
        self.rebind_overlay(bindings)
    }

    /// Binds one resident frame pair for every trajectory descriptor naming it.
    ///
    /// The descriptor declares how many frames a source holds; the binding
    /// supplies the two the renderer samples between. A trajectory whose source
    /// has no matching binding stays unresolved, the same contract a volume
    /// follows. The pair must match the target structure's atom count, which is
    /// checked here because the structure is resolved by the time this runs.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for a malformed pair, a content
    /// hash that is already bound, a pair that contradicts its descriptor, or
    /// frames that do not match the target structure's topology.
    pub fn bind_trajectory(&mut self, binding: TrajectoryBinding) -> Result<(), Error> {
        let mut bindings = self.overlay_bindings.clone();
        bindings.insert_trajectory(binding)?;
        self.rebind_overlay(bindings)
    }

    /// Advances one structure's presentation time inside its resident interval.
    ///
    /// This samples the pair already resident on the GPU, so it uploads no
    /// coordinates: only the interpolation uniform changes.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for a structure this scene does
    /// not carry or a time outside the resident interval.
    pub fn set_trajectory_time(
        &mut self,
        structure: StructureId,
        sample_seconds: f32,
    ) -> Result<(), Error> {
        let handle = self
            .structures
            .keys()
            .copied()
            .zip(self.resolved.structures().map(|(handle, _)| handle))
            .find(|(id, _)| *id == structure)
            .map(|(_, handle)| handle)
            .ok_or_else(|| {
                Error::InvalidSpec("trajectory time targets an unbound structure".to_owned())
            })?;
        self.resolved.set_trajectory_time(handle, sample_seconds)?;
        Ok(())
    }

    /// Plays one structure from a pair of frames, sampling between them at
    /// `sample_seconds`.
    ///
    /// Unlike [`bind_trajectory`](Self::bind_trajectory) this needs no declared
    /// trajectory and does not re-resolve the scene: it swaps the resident pair
    /// (sharing both coordinate arrays with the caller) and refits the
    /// structure's hierarchy, which is what streaming playback needs once per
    /// frame. Moving only the sample time inside the same pair is
    /// [`set_trajectory_time`](Self::set_trajectory_time). The pair is kept, so
    /// an edit that resolves the scene again does not stop the animation.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for a structure this scene does
    /// not carry, a malformed pair, a pair that does not match the structure's
    /// atom count, or a time outside the pair.
    pub fn set_trajectory_pair(
        &mut self,
        structure: StructureId,
        start: TrajectoryFrame,
        end: TrajectoryFrame,
        sample_seconds: f32,
    ) -> Result<(), Error> {
        let pair = PlaybackPair {
            start,
            end,
            sample_seconds,
        };
        self.apply_playback(structure, &pair)?;
        let _ = self.playback.insert(structure, pair);
        Ok(())
    }

    /// Stops playing `structure`, returning it to its own coordinates.
    pub fn clear_trajectory_pair(&mut self, structure: StructureId) {
        if self.playback.remove(&structure).is_some()
            && let Some(handle) = self.structure_handle(structure)
        {
            let _ = self.resolved.clear_trajectory(handle);
        }
    }

    fn structure_handle(&self, structure: StructureId) -> Option<molgfx_core::StructureHandle> {
        self.structures
            .keys()
            .copied()
            .zip(self.resolved.structures().map(|(handle, _)| handle))
            .find(|(id, _)| *id == structure)
            .map(|(_, handle)| handle)
    }

    fn apply_playback(&mut self, structure: StructureId, pair: &PlaybackPair) -> Result<(), Error> {
        let handle = self.structure_handle(structure).ok_or_else(|| {
            Error::InvalidSpec("trajectory pair targets an unbound structure".to_owned())
        })?;
        let segment = molgfx_core::TrajectorySegment::new(
            pair.start.native()?,
            pair.end.native()?,
            pair.sample_seconds,
        )?;
        self.resolved.set_trajectory_segment(handle, segment)?;
        Ok(())
    }

    /// Re-resolves the scene so newly bound runtime data reaches the handle set.
    fn rebind_overlay(&mut self, bindings: crate::overlay::OverlayBindings) -> Result<(), Error> {
        let revision =
            self.spec.revision.checked_add(1).ok_or_else(|| {
                Error::InvalidSpec("scene revision space is exhausted".to_owned())
            })?;
        let resolution = crate::scene::runtime::resolve_reusing(
            &self.spec,
            &self.structures,
            &self.property_bindings,
            &bindings,
            &self.rows,
            Some(&self.structure_assets),
        )?;
        self.overlay_bindings = bindings;
        self.spec.revision = revision;
        self.install_resolution(resolution);
        Ok(())
    }

    /// Renderer-side handle counts for each exposed overlay capability.
    ///
    /// Overlay descriptors reach the renderer when the scene resolves, which
    /// every overlay patch, structure addition and volume binding performs.
    #[must_use]
    pub fn overlay_handles(&self) -> OverlayHandles {
        self.overlay.counts()
    }

    /// Replaces every resolved view of the core scene at once.
    pub(super) fn install_resolution(&mut self, resolution: Resolution) {
        self.structure_assets =
            crate::scene::runtime::StructureAssets::capture(&self.structures, &resolution.scene);
        self.resolved = resolution.scene;
        self.representations = resolution.representations;
        self.selections = resolution.selections;
        self.visuals = resolution.visuals;
        self.properties = resolution.properties;
        self.overlay = resolution.overlay;
        self.appearance = resolution.appearance;
        // A new resolution starts from the structures' own coordinates; put the
        // frames being played back.
        let playing = std::mem::take(&mut self.playback);
        for (structure, pair) in &playing {
            let _ = self.apply_playback(*structure, pair);
        }
        self.playback = playing;
    }
}

/// The two frames a structure is being played from.
#[derive(Clone, Debug)]
pub(super) struct PlaybackPair {
    start: TrajectoryFrame,
    end: TrajectoryFrame,
    sample_seconds: f32,
}
