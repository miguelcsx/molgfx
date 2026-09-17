//! Bounded identity-table preparation for scene rehydration.

use super::{Scene, SceneDescription, invalid};
use crate::handle::SlotMap;

#[cfg(test)]
#[path = "rehydrate_rows_tests.rs"]
mod tests;

pub(super) fn prepare(
    scene: &mut Scene,
    description: &SceneDescription,
) -> Result<(), crate::CoreError> {
    prepare_table(
        &mut scene.structures,
        description.structures.iter().map(|value| value.row),
    )?;
    prepare_table(
        &mut scene.selections,
        description.selections.iter().map(|value| value.row),
    )?;
    prepare_table(
        &mut scene.properties,
        description.atom_properties.iter().map(|value| value.row),
    )?;
    prepare_table(
        &mut scene.representations,
        description.representations.iter().map(|value| value.row),
    )?;
    prepare_table(
        &mut scene.volumes,
        description.volumes.iter().map(|value| value.row),
    )?;
    prepare_table(
        &mut scene.segmentations,
        description.segmentations.iter().map(|value| value.row),
    )?;
    prepare_pickable_table(
        &mut scene.meshes,
        description.meshes.iter().map(|value| value.row),
    )?;
    prepare_table(
        &mut scene.mesh_instances,
        description.mesh_instances.iter().map(|value| value.row),
    )?;
    prepare_pickable_table(
        &mut scene.primitive,
        description.primitives.iter().map(|value| value.row),
    )?;
    prepare_pickable_table(
        &mut scene.ligand_pose_batches,
        description
            .ligand_pose_batches
            .iter()
            .map(|value| value.row),
    )?;
    prepare_pickable_table(
        &mut scene.point_batches,
        description.point_batches.iter().map(|value| value.row),
    )?;
    prepare_pickable_table(
        &mut scene.instance_batches,
        description.instance_batches.iter().map(|value| value.row),
    )?;
    prepare_table(
        &mut scene.attributes,
        description.attributes.iter().map(|value| value.row),
    )?;
    prepare_pickable_table(
        &mut scene.relation_batches,
        description.relation_batches.iter().map(|value| value.row),
    )?;
    prepare_table(
        &mut scene.overlays,
        description.overlays.iter().map(|value| value.row),
    )?;
    prepare_pickable_table(
        &mut scene.guides,
        description.guides.iter().map(|value| value.row),
    )?;
    prepare_pickable_table(
        &mut scene.interactions,
        description.interactions.iter().map(|value| value.row),
    )?;
    prepare_pickable_table(
        &mut scene.labels,
        description.annotations.iter().map(|value| value.row),
    )?;
    prepare_pickable_table(
        &mut scene.labels,
        description.measurements.iter().map(|value| value.row),
    )
}

fn prepare_pickable_table<T, I>(map: &mut SlotMap<T>, rows: I) -> Result<(), crate::CoreError>
where
    I: ExactSizeIterator<Item = u32> + Clone,
{
    if rows.clone().any(|row| row > crate::EntityId::MAX_INDEX) {
        return invalid("manifest row exceeds the picking identity range");
    }
    prepare_table(map, rows)
}

fn prepare_table<T, I>(map: &mut SlotMap<T>, rows: I) -> Result<(), crate::CoreError>
where
    I: ExactSizeIterator<Item = u32>,
{
    match map.prepare_manifest_rows(rows) {
        Ok(()) => Ok(()),
        Err(error) => invalid(error.reason()),
    }
}
