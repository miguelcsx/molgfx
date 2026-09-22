//! Ordered patch application for scientific semantic items.

use crate::scene::runtime::{insert_unique, remove_existing};
use crate::{Error, PatchOperation, SceneSpec};

pub(crate) fn apply(candidate: &mut SceneSpec, operation: &PatchOperation) -> Result<bool, Error> {
    match operation {
        PatchOperation::AddVolume { id, volume } => {
            volume.validate()?;
            insert_unique(&mut candidate.volumes, *id, volume.clone(), "volume")?;
        }
        PatchOperation::RemoveVolume { id } => remove_existing(&mut candidate.volumes, id)?,
        PatchOperation::AddAnnotation { id, annotation } => {
            annotation.validate(candidate)?;
            insert_unique(
                &mut candidate.annotations,
                *id,
                annotation.clone(),
                "annotation",
            )?;
        }
        PatchOperation::RemoveAnnotation { id } => {
            remove_existing(&mut candidate.annotations, id)?;
        }
        PatchOperation::AddMeasurement { id, measurement } => {
            measurement.validate(candidate)?;
            insert_unique(
                &mut candidate.measurements,
                *id,
                measurement.clone(),
                "measurement",
            )?;
        }
        PatchOperation::RemoveMeasurement { id } => {
            remove_existing(&mut candidate.measurements, id)?;
        }
        PatchOperation::AddScientificInteraction { id, interaction } => {
            interaction.validate(candidate)?;
            insert_unique(
                &mut candidate.scientific_interactions,
                *id,
                interaction.clone(),
                "interaction",
            )?;
        }
        PatchOperation::RemoveScientificInteraction { id } => {
            remove_existing(&mut candidate.scientific_interactions, id)?;
        }
        PatchOperation::AddTrajectory { id, trajectory } => {
            trajectory.validate(candidate)?;
            insert_unique(
                &mut candidate.trajectories,
                *id,
                trajectory.clone(),
                "trajectory",
            )?;
        }
        PatchOperation::RemoveTrajectory { id } => {
            remove_existing(&mut candidate.trajectories, id)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}
