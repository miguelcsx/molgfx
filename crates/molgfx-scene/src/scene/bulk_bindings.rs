//! Borrowed runtime asset views for transport and persistence, in descriptor order.

use crate::{
    HydrogenBondBinding, InteractionId, MolecularSurfaceBinding, RepresentationId, Scene,
    VolumeBinding, VolumeId,
};

impl Scene {
    /// Captures imperative playback intervals without copying any coordinate columns.
    pub fn playback_bindings(&self) -> impl Iterator<Item = super::PlaybackBinding> + '_ {
        self.playback
            .iter()
            .map(|(&structure, pair)| super::PlaybackBinding {
                structure,
                start: pair.start.clone(),
                end: pair.end.clone(),
                sample_seconds: pair.sample_seconds,
            })
    }
    /// Borrows resident trajectory bindings in descriptor identity order.
    pub fn bound_trajectories(
        &self,
    ) -> impl Iterator<Item = (crate::TrajectoryId, &crate::TrajectoryBinding)> {
        self.spec.trajectories.iter().filter_map(|(&id, spec)| {
            self.overlay_bindings
                .trajectory(&spec.source.content_hash)
                .map(|binding| (id, binding))
        })
    }

    /// Borrows bound scalar grids without copying or embedding them in the scene document.
    pub fn bound_volumes(&self) -> impl Iterator<Item = (VolumeId, &VolumeBinding)> {
        self.spec.volumes.iter().filter_map(|(&id, spec)| {
            self.overlay_bindings
                .volume(&spec.source.content_hash)
                .map(|binding| (id, binding))
        })
    }

    /// Borrows chemically classified interaction tables in authored identity order.
    pub fn bound_hydrogen_bonds(
        &self,
    ) -> impl Iterator<Item = (InteractionId, &HydrogenBondBinding)> {
        self.spec.interactions.iter().filter_map(|(&id, spec)| {
            let crate::InteractionSpec::HydrogenBonds(spec) = spec else {
                return None;
            };
            self.overlay_bindings
                .hydrogen_bonds(&spec.source.content_hash)
                .map(|binding| (id, binding))
        })
    }

    /// Borrows scientific surfaces in representation order, retaining their shared provider owners.
    pub fn bound_molecular_surfaces(
        &self,
    ) -> impl Iterator<Item = (RepresentationId, &MolecularSurfaceBinding)> {
        self.spec.representations.iter().filter_map(|(&id, spec)| {
            let crate::representation::form::RepresentationFormSpec::MolecularSurface {
                source,
                ..
            } = &spec.form
            else {
                return None;
            };
            self.overlay_bindings
                .molecular_surface(&source.content_hash, spec.structure_id()?)
                .map(|binding| (id, binding))
        })
    }
}
