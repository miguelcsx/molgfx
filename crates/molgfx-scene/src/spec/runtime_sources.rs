//! Runtime asset identities can be inspected without resolving geometry or cloning columns.

use crate::{DataSource, InteractionSpec, PatchOperation, SceneSpec};

impl SceneSpec {
    /// Borrows scientific asset identities referenced by this document.
    /// Coordinate owners and bulk data remain outside the document.
    pub fn runtime_sources(&self) -> impl Iterator<Item = &DataSource> {
        self.volumes
            .values()
            .map(|value| &value.source)
            .chain(self.segmentations.values().map(|value| &value.source))
            .chain(self.trajectories.values().map(|value| &value.source))
            .chain(self.interactions.values().filter_map(interaction_source))
            .chain(self.representations.values().filter_map(|value| {
                value
                    .molecular_surface_recipe()
                    .map(|(source, _, _)| source)
            }))
    }
}

impl PatchOperation {
    /// Borrows scientific input that applying or reversing this operation can require.
    pub fn runtime_sources(&self) -> impl Iterator<Item = &DataSource> {
        let direct = match self {
            Self::AddVolume { volume, .. } => Some(&volume.source),
            Self::AddSegmentation { segmentation, .. } => Some(&segmentation.source),
            Self::AddTrajectory { trajectory, .. } => Some(&trajectory.source),
            Self::AddInteraction { interaction, .. } => interaction_source(interaction),
            Self::AddRepresentation { representation, .. }
            | Self::ReplaceRepresentation { representation, .. } => representation
                .molecular_surface_recipe()
                .map(|(source, _, _)| source),
            _ => None,
        };
        let snapshot = match self {
            Self::RestoreSnapshot(snapshot) => Some(&snapshot.scene),
            _ => None,
        };
        direct
            .into_iter()
            .chain(snapshot.into_iter().flat_map(SceneSpec::runtime_sources))
    }
}

fn interaction_source(value: &InteractionSpec) -> Option<&DataSource> {
    match value {
        InteractionSpec::HydrogenBonds(value) => Some(&value.source),
        InteractionSpec::Explicit { .. } => None,
    }
}
