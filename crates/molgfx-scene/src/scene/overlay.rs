//! Runtime overlay bindings and renderer-handle inspection.

use super::Scene;
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
            .chain(
                self.spec
                    .interactions
                    .iter()
                    .filter_map(|(id, interaction)| {
                        let crate::InteractionSpec::HydrogenBonds(spec) = interaction else {
                            return None;
                        };
                        self.overlay_bindings
                            .hydrogen_bonds(&spec.source.content_hash)
                            .is_none()
                            .then_some(crate::OverlayId::Interaction(*id))
                    }),
            )
            .collect()
    }
    /// Representations waiting for analytical provider geometry.
    #[must_use]
    pub fn unresolved_representations(&self) -> Vec<crate::RepresentationId> {
        self.spec
            .representations
            .iter()
            .filter_map(|(id, spec)| {
                let crate::representation::form::RepresentationFormSpec::MolecularSurface {
                    source,
                    ..
                } = &spec.form
                else {
                    return None;
                };
                spec.structure_id()
                    .and_then(|structure| {
                        self.overlay_bindings
                            .molecular_surface(&source.content_hash, structure)
                    })
                    .is_none()
                    .then_some(*id)
            })
            .collect()
    }

    /// Binds a shared analytical surface after checking molecule, pose and controls.
    ///
    /// # Errors
    /// Rejects conflicting sources or mismatched descriptors atomically.
    pub fn bind_molecular_surface(
        &mut self,
        binding: crate::representation::MolecularSurfaceBinding,
    ) -> Result<(), Error> {
        let molecule = self
            .structures
            .get(&binding.structure)
            .ok_or(Error::MissingId)?;
        binding.validate_molecule(molecule)?;
        self.validate_surface_pose(&binding)?;
        let mut bindings = self.overlay_bindings.clone();
        bindings.insert_molecular_surface(binding)?;
        self.rebind_overlay(bindings)
    }

    /// Binds chemically classified hydrogen bonds to live atom endpoints.
    /// Captures the current input when no pose is supplied. Background results
    /// must retain their calculation input with [`crate::HydrogenBondBinding::at_pose`].
    ///
    /// # Errors
    /// Rejects malformed geometry, conflicting source identities and atom rows
    /// outside a descriptor's structure without changing the previous scene.
    pub fn bind_hydrogen_bonds(
        &mut self,
        binding: crate::HydrogenBondBinding,
    ) -> Result<(), Error> {
        let binding = self.validate_hydrogen_bond_input(binding, &self.spec)?;
        let mut bindings = self.overlay_bindings.clone();
        bindings.insert_hydrogen_bonds(binding)?;
        self.rebind_overlay(bindings)
    }

    /// Binds a shared scalar grid after validating every affected descriptor.
    ///
    /// # Errors
    /// Returns an error for invalid or conflicting grid data.
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
        sample_seconds: f64,
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
        if let Some(pair) = self.playback.get_mut(&structure) {
            pair.sample_seconds = sample_seconds;
        } else if let Some(segment) = self
            .resolved
            .structure(handle)
            .and_then(molgfx_core::PlacedStructure::trajectory)
        {
            self.overlay_bindings.retain_trajectory_time(segment);
        }
        Ok(())
    }

    /// Plays one structure from a pair of frames, sampling between them at
    /// `sample_seconds`.
    ///
    /// Unlike [`bind_trajectory`](Self::bind_trajectory) this needs no declared
    /// trajectory and does not re-resolve the scene: it swaps the resident pair
    /// (sharing both coordinate arrays with the caller) and refits the
    /// structure's hierarchy once per interval. Supplying the same retained
    /// pair only updates time, without scanning coordinates or refitting bounds.
    /// Moving only the sample time inside the same pair is
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
        sample_seconds: f64,
    ) -> Result<(), Error> {
        if self
            .playback
            .get(&structure)
            .is_some_and(|pair| pair.start.same_frame(&start) && pair.end.same_frame(&end))
        {
            return self.set_trajectory_time(structure, sample_seconds);
        }
        let pair = PlaybackPair {
            start,
            end,
            sample_seconds,
            source: self
                .structures
                .get(&structure)
                .map(|source| (source.identity(), source.coordinate_revision()))
                .ok_or_else(|| {
                    Error::InvalidSpec("trajectory pair targets an unbound structure".to_owned())
                })?,
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

    /// Whether this molecular instance is sampling a resident trajectory pair.
    #[must_use]
    pub fn has_trajectory_pair(&self, structure: StructureId) -> bool {
        self.structure_handle(structure)
            .and_then(|handle| self.resolved.structure(handle))
            .is_some_and(|placed| placed.trajectory().is_some())
    }

    /// Process-local cache identity of the resident frames and displayed time.
    /// Retained coordinate storage is hashed by address; this key is not portable.
    #[must_use]
    pub fn trajectory_pose_key(&self, structure: StructureId) -> Option<u64> {
        use std::hash::{Hash as _, Hasher as _};
        let segment = self
            .structure_handle(structure)
            .and_then(|handle| self.resolved.structure(handle))?
            .trajectory()?;
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for frame in [segment.start(), segment.end()] {
            frame.index().hash(&mut hash);
            frame.time_seconds().to_bits().hash(&mut hash);
            frame.positions().as_ptr().hash(&mut hash);
        }
        segment.sample_seconds().to_bits().hash(&mut hash);
        Some(hash.finish())
    }

    pub(super) fn structure_handle(
        &self,
        structure: StructureId,
    ) -> Option<molgfx_core::StructureHandle> {
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
        let resolution = self.prepare_resolution_playback(resolution, &self.structures)?;
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
    pub(super) fn install_resolution(
        &mut self,
        prepared: super::playback_resolution::PreparedResolution,
    ) {
        let resolution = prepared.resolution;
        self.structure_assets =
            crate::scene::runtime::StructureAssets::capture(&self.structures, &resolution.scene);
        self.resolved = resolution.scene;
        self.representations = resolution.representations;
        self.selections = resolution.selections;
        self.visuals = resolution.visuals;
        self.properties = resolution.properties;
        self.overlay = resolution.overlay;
        self.appearance = resolution.appearance;
        self.playback = prepared.playback;
    }
}

/// The two frames a structure is being played from.
#[derive(Clone, Debug)]
pub(super) struct PlaybackPair {
    pub(super) source: (u64, u64),
    pub(super) start: TrajectoryFrame,
    pub(super) end: TrajectoryFrame,
    pub(super) sample_seconds: f64,
}
