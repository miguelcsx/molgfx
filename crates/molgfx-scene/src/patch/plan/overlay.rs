//! Prepared overlay-domain table updates.

use super::OverlayDomains;
use crate::error::{Error, PatchError};
use crate::spec::PatchOperation;
use crate::spec::SceneSpec;
use std::collections::BTreeMap;

impl OverlayDomains {
    pub(super) fn apply(
        &mut self,
        spec: &SceneSpec,
        operation: &PatchOperation,
    ) -> Result<bool, Error> {
        match operation {
            PatchOperation::AddVolume { id, volume } => {
                volume.validate()?;
                insert_item(&mut self.volumes, &spec.volumes, *id, volume.clone())?;
            }
            PatchOperation::RemoveVolume { id } => {
                remove(self.volumes.get_or_insert_with(|| spec.volumes.clone()), id)?;
            }
            PatchOperation::SetVolumeIsovalue { id, isovalue } => {
                if !isovalue.is_finite() {
                    return Err(Error::InvalidSpec(
                        "volume isovalue must be finite".to_owned(),
                    ));
                }
                self.volumes
                    .get_or_insert_with(|| spec.volumes.clone())
                    .get_mut(id)
                    .ok_or(PatchError::MissingId)?
                    .isovalue = *isovalue;
            }
            PatchOperation::AddAnnotation { id, annotation } => {
                annotation.validate(spec)?;
                insert_item(
                    &mut self.annotations,
                    &spec.annotations,
                    *id,
                    annotation.clone(),
                )?;
            }
            PatchOperation::RemoveAnnotation { id } => {
                remove_item(&mut self.annotations, &spec.annotations, id)?;
            }
            PatchOperation::AddMeasurement { id, measurement } => {
                measurement.validate(spec)?;
                insert_item(
                    &mut self.measurements,
                    &spec.measurements,
                    *id,
                    measurement.clone(),
                )?;
            }
            PatchOperation::RemoveMeasurement { id } => {
                remove_item(&mut self.measurements, &spec.measurements, id)?;
            }
            PatchOperation::AddInteraction { id, interaction } => {
                interaction.validate(spec)?;
                insert_item(
                    &mut self.interactions,
                    &spec.interactions,
                    *id,
                    interaction.clone(),
                )?;
            }
            PatchOperation::RemoveInteraction { id } => {
                remove_item(&mut self.interactions, &spec.interactions, id)?;
            }
            PatchOperation::AddTrajectory { id, trajectory } => {
                trajectory.validate(spec)?;
                insert_item(
                    &mut self.trajectories,
                    &spec.trajectories,
                    *id,
                    trajectory.clone(),
                )?;
            }
            PatchOperation::RemoveTrajectory { id } => {
                remove_item(&mut self.trajectories, &spec.trajectories, id)?;
            }
            PatchOperation::AddEllipsoids { id, spec: item } => {
                item.validate(spec)?;
                insert_item(&mut self.ellipsoids, &spec.ellipsoids, *id, item.clone())?;
            }
            PatchOperation::RemoveEllipsoids { id } => {
                remove_item(&mut self.ellipsoids, &spec.ellipsoids, id)?;
            }
            PatchOperation::AddPlane { id, spec: item } => {
                item.validate(spec)?;
                insert_item(&mut self.planes, &spec.planes, *id, *item)?;
            }
            PatchOperation::RemovePlane { id } => {
                remove_item(&mut self.planes, &spec.planes, id)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn commit(self, spec: &mut SceneSpec) {
        replace(&mut spec.volumes, self.volumes);
        replace(&mut spec.annotations, self.annotations);
        replace(&mut spec.measurements, self.measurements);
        replace(&mut spec.interactions, self.interactions);
        replace(&mut spec.trajectories, self.trajectories);
        replace(&mut spec.ellipsoids, self.ellipsoids);
        replace(&mut spec.planes, self.planes);
    }
}

/// Inserts one overlay value, materialising the table copy on first touch.
fn insert_item<K: Ord + Clone, V: Clone>(
    table: &mut Option<BTreeMap<K, V>>,
    source: &BTreeMap<K, V>,
    id: K,
    value: V,
) -> Result<(), Error> {
    insert_new(table.get_or_insert_with(|| source.clone()), id, value)
}

/// Removes one overlay value, materialising the table copy on first touch.
fn remove_item<K: Ord + Clone, V: Clone>(
    table: &mut Option<BTreeMap<K, V>>,
    source: &BTreeMap<K, V>,
    id: &K,
) -> Result<(), Error> {
    remove(table.get_or_insert_with(|| source.clone()), id)
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
