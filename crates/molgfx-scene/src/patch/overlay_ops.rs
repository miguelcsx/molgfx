//! Ordered patch application for overlay items.
//! Segmentation lifetimes and volume presentations validate in their own handlers.

use crate::scene::apply::{insert_unique, remove_existing};
use crate::{Error, PatchOperation, SceneSpec};

pub(crate) fn apply(candidate: &mut SceneSpec, operation: &PatchOperation) -> Result<bool, Error> {
    if apply_segmentation(candidate, operation)? || apply_volume(candidate, operation)? {
        return Ok(true);
    }
    apply_overlay_item(candidate, operation)
}

fn apply_segmentation(
    candidate: &mut SceneSpec,
    operation: &PatchOperation,
) -> Result<bool, Error> {
    match operation {
        PatchOperation::AddSegmentation { id, segmentation } => {
            segmentation.validate()?;
            if id.index == 0
                || id.generation == 0
                || candidate
                    .segmentations
                    .keys()
                    .any(|live| live.index == id.index)
            {
                return Err(Error::InvalidSpec(
                    "segmentation slot is invalid or already occupied".into(),
                ));
            }
            insert_unique(
                &mut candidate.segmentations,
                *id,
                segmentation.clone(),
                "segmentation",
            )?;
            candidate
                .segmentation_generations
                .entry(id.index)
                .and_modify(|generation| *generation = (*generation).max(id.generation))
                .or_insert(id.generation);
        }
        PatchOperation::RemoveSegmentation { id } => {
            remove_existing(&mut candidate.segmentations, id)?;
            candidate
                .segmentation_generations
                .entry(id.index)
                .and_modify(|generation| *generation = (*generation).max(id.generation))
                .or_insert(id.generation);
        }
        PatchOperation::SetSegmentStyles { id, styles } => {
            let _ = crate::overlay::segmentation_spec::native_styles(styles)?;
            candidate
                .segmentations
                .get_mut(id)
                .ok_or(crate::PatchError::MissingId)?
                .styles
                .clone_from(styles);
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_volume(candidate: &mut SceneSpec, operation: &PatchOperation) -> Result<bool, Error> {
    match operation {
        PatchOperation::AddVolume { id, volume } => {
            volume.validate()?;
            insert_unique(&mut candidate.volumes, *id, volume.clone(), "volume")?;
        }
        PatchOperation::RemoveVolume { id } => remove_existing(&mut candidate.volumes, id)?,
        PatchOperation::SetVolumeIsovalue { id, isovalue } => {
            if !isovalue.is_finite() {
                return Err(Error::InvalidSpec(
                    "volume isovalue must be finite".to_owned(),
                ));
            }
            candidate
                .volumes
                .get_mut(id)
                .ok_or(crate::PatchError::MissingId)?
                .set_isovalue(*isovalue)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_overlay_item(
    candidate: &mut SceneSpec,
    operation: &PatchOperation,
) -> Result<bool, Error> {
    match operation {
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
        PatchOperation::AddInteraction { id, interaction } => {
            interaction.validate(candidate)?;
            insert_unique(
                &mut candidate.interactions,
                *id,
                interaction.clone(),
                "interaction",
            )?;
        }
        PatchOperation::RemoveInteraction { id } => {
            remove_existing(&mut candidate.interactions, id)?;
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
        PatchOperation::AddEllipsoids { id, spec } => {
            spec.validate(candidate)?;
            insert_unique(&mut candidate.ellipsoids, *id, spec.clone(), "ellipsoid")?;
        }
        PatchOperation::RemoveEllipsoids { id } => {
            remove_existing(&mut candidate.ellipsoids, id)?;
        }
        PatchOperation::AddPlane { id, spec } => {
            spec.validate(candidate)?;
            insert_unique(&mut candidate.planes, *id, *spec, "plane")?;
        }
        PatchOperation::RemovePlane { id } => remove_existing(&mut candidate.planes, id)?,
        _ => return Ok(false),
    }
    Ok(true)
}

#[cfg(test)]
#[path = "overlay_ops_tests.rs"]
mod tests;
