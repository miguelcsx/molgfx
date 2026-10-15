//! Inverse operations: what undoes each semantic change against its base.
//! Segmentation lifetimes, volume presentations and overlay items invert separately.

use std::collections::BTreeMap;

use crate::PatchOperation;
use crate::spec::SceneSpec;

type Inverse = Result<Vec<PatchOperation>, crate::Error>;

pub(super) fn inverse_operations(operation: &PatchOperation, base: &SceneSpec) -> Inverse {
    if let Some(operations) = inverse_overlay(operation, base)? {
        return Ok(operations);
    }
    if let Some(operations) = inverse_representation(operation, base)? {
        return Ok(operations);
    }
    if let Some(operations) = inverse_appearance(operation, base)? {
        return Ok(operations);
    }
    if let Some(operations) = inverse_domain(operation, base)? {
        return Ok(operations);
    }
    let one = |operation| vec![operation];
    Ok(match operation {
        PatchOperation::RestoreSnapshot(_) => one(PatchOperation::RestoreSnapshot(Box::new(
            crate::snapshot::SceneSnapshot::capture(base),
        ))),
        PatchOperation::AddStructure { id, .. } => vec![PatchOperation::AddStructure {
            id: *id,
            source: fetch(&base.structures, id)?.clone(),
        }],
        PatchOperation::SetFocus { .. } => one(PatchOperation::SetFocus {
            selection: base.focus.clone(),
        }),
        PatchOperation::SetInteraction { channel, .. } => one(PatchOperation::SetInteraction {
            channel: channel.clone(),
            selection: base.interaction(channel).cloned(),
        }),
        PatchOperation::SetCamera { .. } => one(PatchOperation::SetCamera {
            camera: base.camera,
        }),
        _ => {
            return Err(crate::Error::InvalidSpec(
                "patch inversion was not dispatched".to_owned(),
            ));
        }
    })
}

fn inverse_representation(
    operation: &PatchOperation,
    base: &SceneSpec,
) -> Result<Option<Vec<PatchOperation>>, crate::Error> {
    Ok(match operation {
        PatchOperation::AddRepresentation { id, .. } => {
            one(PatchOperation::RemoveRepresentation { id: *id })
        }
        PatchOperation::RemoveRepresentation { id } => one(PatchOperation::AddRepresentation {
            id: *id,
            representation: fetch(&base.representations, id)?.clone(),
        }),
        PatchOperation::ReplaceRepresentation { id, .. } => {
            one(PatchOperation::ReplaceRepresentation {
                id: *id,
                representation: fetch(&base.representations, id)?.clone(),
            })
        }
        PatchOperation::SetRepresentationTarget { id, .. } => {
            one(PatchOperation::SetRepresentationTarget {
                id: *id,
                target: fetch(&base.representations, id)?.common.target.clone(),
            })
        }
        PatchOperation::SetColor { id, .. } => one(PatchOperation::SetColor {
            id: *id,
            color: fetch(&base.representations, id)?.common.color.clone(),
        }),
        PatchOperation::SetVisibility { id, .. } => one(PatchOperation::SetVisibility {
            id: *id,
            visible: fetch(&base.representations, id)?.common.visible,
        }),
        PatchOperation::SetOpacity { id, .. } => one(PatchOperation::SetOpacity {
            id: *id,
            opacity: fetch(&base.representations, id)?.opacity(),
        }),
        PatchOperation::SetVisual { id, .. } => {
            let representation = fetch(&base.representations, id)?;
            let mut operations = Vec::with_capacity(representation.common.parameters.len() + 1);
            operations.push(PatchOperation::SetVisual {
                id: *id,
                visual: representation.common.visual.clone(),
            });
            operations.extend(
                representation
                    .common
                    .parameters
                    .iter()
                    .map(|(name, value)| PatchOperation::SetParameter {
                        id: *id,
                        name: name.clone(),
                        value: Some(value.clone()),
                    }),
            );
            Some(operations)
        }
        PatchOperation::SetParameter { id, name, .. } => one(PatchOperation::SetParameter {
            id: *id,
            name: name.clone(),
            value: fetch(&base.representations, id)?
                .common
                .parameters
                .get(name)
                .cloned(),
        }),
        _ => None,
    })
}

fn inverse_overlay(
    operation: &PatchOperation,
    base: &SceneSpec,
) -> Result<Option<Vec<PatchOperation>>, crate::Error> {
    if let Some(operations) = inverse_segmentation(operation, base)? {
        return Ok(Some(operations));
    }
    if let Some(operations) = inverse_volume(operation, base)? {
        return Ok(Some(operations));
    }
    inverse_overlay_item(operation, base)
}

fn inverse_segmentation(
    operation: &PatchOperation,
    base: &SceneSpec,
) -> Result<Option<Vec<PatchOperation>>, crate::Error> {
    Ok(match operation {
        PatchOperation::AddSegmentation { id, .. } => {
            one(PatchOperation::RemoveSegmentation { id: *id })
        }
        PatchOperation::RemoveSegmentation { id } => one(PatchOperation::AddSegmentation {
            id: *id,
            segmentation: fetch(&base.segmentations, id)?.clone(),
        }),
        PatchOperation::SetSegmentStyles { id, .. } => one(PatchOperation::SetSegmentStyles {
            id: *id,
            styles: fetch(&base.segmentations, id)?.styles.clone(),
        }),
        _ => None,
    })
}

fn inverse_volume(
    operation: &PatchOperation,
    base: &SceneSpec,
) -> Result<Option<Vec<PatchOperation>>, crate::Error> {
    Ok(match operation {
        PatchOperation::AddVolume { id, .. } => one(PatchOperation::RemoveVolume { id: *id }),
        PatchOperation::RemoveVolume { id } => one(PatchOperation::AddVolume {
            id: *id,
            volume: fetch(&base.volumes, id)?.clone(),
        }),
        PatchOperation::SetVolumeIsovalue { id, .. } => one(PatchOperation::SetVolumeIsovalue {
            id: *id,
            isovalue: fetch(&base.volumes, id)?.isovalue().ok_or_else(|| {
                crate::Error::InvalidSpec("volume has no isosurface presentation".to_owned())
            })?,
        }),
        _ => None,
    })
}

fn inverse_overlay_item(
    operation: &PatchOperation,
    base: &SceneSpec,
) -> Result<Option<Vec<PatchOperation>>, crate::Error> {
    Ok(match operation {
        PatchOperation::AddAnnotation { id, .. } => {
            one(PatchOperation::RemoveAnnotation { id: *id })
        }
        PatchOperation::RemoveAnnotation { id } => one(PatchOperation::AddAnnotation {
            id: *id,
            annotation: fetch(&base.annotations, id)?.clone(),
        }),
        PatchOperation::AddEllipsoids { id, .. } => {
            one(PatchOperation::RemoveEllipsoids { id: *id })
        }
        PatchOperation::RemoveEllipsoids { id } => one(PatchOperation::AddEllipsoids {
            id: *id,
            spec: fetch(&base.ellipsoids, id)?.clone(),
        }),
        PatchOperation::AddPlane { id, .. } => one(PatchOperation::RemovePlane { id: *id }),
        PatchOperation::RemovePlane { id } => one(PatchOperation::AddPlane {
            id: *id,
            spec: *fetch(&base.planes, id)?,
        }),
        PatchOperation::AddMeasurement { id, .. } => {
            one(PatchOperation::RemoveMeasurement { id: *id })
        }
        PatchOperation::RemoveMeasurement { id } => one(PatchOperation::AddMeasurement {
            id: *id,
            measurement: fetch(&base.measurements, id)?.clone(),
        }),
        PatchOperation::AddInteraction { id, .. } => {
            one(PatchOperation::RemoveInteraction { id: *id })
        }
        PatchOperation::RemoveInteraction { id } => one(PatchOperation::AddInteraction {
            id: *id,
            interaction: fetch(&base.interactions, id)?.clone(),
        }),
        PatchOperation::AddTrajectory { id, .. } => {
            one(PatchOperation::RemoveTrajectory { id: *id })
        }
        PatchOperation::RemoveTrajectory { id } => one(PatchOperation::AddTrajectory {
            id: *id,
            trajectory: fetch(&base.trajectories, id)?.clone(),
        }),
        _ => None,
    })
}

fn inverse_domain(
    operation: &PatchOperation,
    base: &SceneSpec,
) -> Result<Option<Vec<PatchOperation>>, crate::Error> {
    let restore = |key: &str| base.extensions.get(key).cloned();
    Ok(match operation {
        PatchOperation::SetAssembly { .. } => one(PatchOperation::SetAssembly {
            assembly: base.assembly.clone(),
        }),
        PatchOperation::SetFitting { .. } => one(PatchOperation::SetFitting {
            fitting: restore("molgfx.fitting")
                .map(serde_json::from_value)
                .transpose()
                .map_err(crate::Error::from)?,
        }),
        PatchOperation::SetValidation { .. } => one(PatchOperation::SetValidation {
            findings: restore("molgfx.validation")
                .map_or_else(|| Ok(Vec::new()), serde_json::from_value)
                .map_err(crate::Error::from)?,
        }),
        _ => None,
    })
}

fn inverse_appearance(
    operation: &PatchOperation,
    base: &SceneSpec,
) -> Result<Option<Vec<PatchOperation>>, crate::Error> {
    let rule = |id| Ok::<_, crate::Error>(fetch(&base.appearance, id)?.clone());
    Ok(match operation {
        PatchOperation::AddAppearanceRule { id, .. } => {
            one(PatchOperation::RemoveAppearanceRule { id: *id })
        }
        PatchOperation::ReplaceAppearanceRule { id, .. } => {
            one(PatchOperation::ReplaceAppearanceRule {
                id: *id,
                rule: rule(id)?,
            })
        }
        PatchOperation::RemoveAppearanceRule { id } => one(PatchOperation::AddAppearanceRule {
            id: *id,
            rule: rule(id)?,
        }),
        _ => None,
    })
}

/// The value stored under `id`, or the missing-id error.
fn fetch<'a, K: Ord, V>(map: &'a BTreeMap<K, V>, id: &K) -> Result<&'a V, crate::Error> {
    Ok(map.get(id).ok_or(crate::PatchError::MissingId)?)
}

fn one<T: From<Vec<PatchOperation>>>(operation: PatchOperation) -> T {
    T::from(vec![operation])
}
