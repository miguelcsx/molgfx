//! Ordered patch application for overlay items.

use crate::scene::apply::{insert_unique, remove_existing};
use crate::{Error, PatchOperation, SceneSpec};

pub(crate) fn apply(candidate: &mut SceneSpec, operation: &PatchOperation) -> Result<bool, Error> {
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
                .isovalue = *isovalue;
        }
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
        _ => return Ok(false),
    }
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::apply;
    use crate::{Color, DataSource, PatchOperation, SceneSpec, VolumeSpec};

    #[test]
    fn set_isovalue_rejects_nonfinite_before_mutating() {
        let id = crate::VolumeId::new(1);
        let mut spec = SceneSpec::empty();
        spec.volumes.insert(
            id,
            VolumeSpec {
                source: DataSource::new("density"),
                dimensions: [2, 2, 2],
                spacing: [1.0; 3],
                origin: [0.0; 3],
                isovalue: 1.0,
                color: Color::rgb(1, 2, 3),
            },
        );
        let before = spec.clone();
        assert!(
            apply(
                &mut spec,
                &PatchOperation::SetVolumeIsovalue {
                    id,
                    isovalue: f32::NAN
                }
            )
            .is_err()
        );
        assert_eq!(spec, before);
    }
}
