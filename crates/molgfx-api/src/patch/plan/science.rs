//! Prepared scientific-domain table updates.

use super::{SceneSpec, ScienceDomains};
use crate::error::{Error, PatchError};
use crate::spec::PatchOperation;
use std::collections::BTreeMap;

impl ScienceDomains {
    pub(super) fn apply(
        &mut self,
        spec: &SceneSpec,
        operation: &PatchOperation,
    ) -> Result<bool, Error> {
        match operation {
            PatchOperation::AddVolume { id, volume } => {
                volume.validate()?;
                insert_new(
                    self.volumes.get_or_insert_with(|| spec.volumes.clone()),
                    *id,
                    volume.clone(),
                )?;
            }
            PatchOperation::RemoveVolume { id } => {
                remove(self.volumes.get_or_insert_with(|| spec.volumes.clone()), id)?;
            }
            PatchOperation::AddAnnotation { id, annotation } => {
                annotation.validate(spec)?;
                insert_new(
                    self.annotations
                        .get_or_insert_with(|| spec.annotations.clone()),
                    *id,
                    annotation.clone(),
                )?;
            }
            PatchOperation::RemoveAnnotation { id } => {
                remove(
                    self.annotations
                        .get_or_insert_with(|| spec.annotations.clone()),
                    id,
                )?;
            }
            PatchOperation::AddMeasurement { id, measurement } => {
                measurement.validate(spec)?;
                insert_new(
                    self.measurements
                        .get_or_insert_with(|| spec.measurements.clone()),
                    *id,
                    measurement.clone(),
                )?;
            }
            PatchOperation::RemoveMeasurement { id } => {
                remove(
                    self.measurements
                        .get_or_insert_with(|| spec.measurements.clone()),
                    id,
                )?;
            }
            PatchOperation::AddScientificInteraction { id, interaction } => {
                interaction.validate(spec)?;
                insert_new(
                    self.interactions
                        .get_or_insert_with(|| spec.scientific_interactions.clone()),
                    *id,
                    interaction.clone(),
                )?;
            }
            PatchOperation::RemoveScientificInteraction { id } => {
                remove(
                    self.interactions
                        .get_or_insert_with(|| spec.scientific_interactions.clone()),
                    id,
                )?;
            }
            PatchOperation::AddTrajectory { id, trajectory } => {
                trajectory.validate(spec)?;
                insert_new(
                    self.trajectories
                        .get_or_insert_with(|| spec.trajectories.clone()),
                    *id,
                    trajectory.clone(),
                )?;
            }
            PatchOperation::RemoveTrajectory { id } => {
                remove(
                    self.trajectories
                        .get_or_insert_with(|| spec.trajectories.clone()),
                    id,
                )?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn commit(self, spec: &mut SceneSpec) {
        replace(&mut spec.volumes, self.volumes);
        replace(&mut spec.annotations, self.annotations);
        replace(&mut spec.measurements, self.measurements);
        replace(&mut spec.scientific_interactions, self.interactions);
        replace(&mut spec.trajectories, self.trajectories);
    }
}

fn insert_new<K: Ord, V>(values: &mut BTreeMap<K, V>, id: K, value: V) -> Result<(), Error> {
    if values.insert(id, value).is_some() {
        return Err(PatchError::Invalid("semantic ID already exists".to_owned()).into());
    }
    Ok(())
}

fn remove<K: Ord, V>(values: &mut BTreeMap<K, V>, id: &K) -> Result<(), Error> {
    if values.remove(id).is_none() {
        return Err(PatchError::MissingId.into());
    }
    Ok(())
}

fn replace<K: Ord, V>(target: &mut BTreeMap<K, V>, update: Option<BTreeMap<K, V>>) {
    if let Some(update) = update {
        *target = update;
    }
}
