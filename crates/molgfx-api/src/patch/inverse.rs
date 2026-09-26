//! Inverse operations: what undoes each semantic change against its base.

use crate::PatchOperation;
use crate::id::StructureId;
use crate::representation::Selection;
use crate::spec::{InteractionChannel, SceneSpec};

type Inverse = Result<Vec<PatchOperation>, crate::Error>;

pub(super) fn inverse_operations(operation: &PatchOperation, base: &SceneSpec) -> Inverse {
    if let Some(operations) = inverse_science(operation, base)? {
        return Ok(operations);
    }
    if let Some(operations) = inverse_representation(operation, base)? {
        return Ok(operations);
    }
    if let Some(operations) = inverse_appearance(operation, base)? {
        return Ok(operations);
    }
    let one = |operation| vec![operation];
    let structure = |id: StructureId, base: &SceneSpec| {
        let source = base
            .structures
            .get(&id)
            .cloned()
            .ok_or(crate::PatchError::MissingId);
        source.map(|source| vec![PatchOperation::AddStructure { id, source }])
    };
    Ok(match operation {
        PatchOperation::AddStructure { id, .. } => structure(*id, base)?,
        PatchOperation::SetFocus { .. } => one(PatchOperation::SetFocus {
            selection: base.focus.clone(),
        }),
        PatchOperation::SetInteraction { channel, .. } => one(PatchOperation::SetInteraction {
            channel: channel.clone(),
            selection: interaction(base, channel).cloned(),
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
    let one = |operation| Some(vec![operation]);
    Ok(match operation {
        PatchOperation::AddRepresentation { id, .. } => {
            one(PatchOperation::RemoveRepresentation { id: *id })
        }
        PatchOperation::RemoveRepresentation { id } => one(PatchOperation::AddRepresentation {
            id: *id,
            representation: base
                .representations
                .get(id)
                .cloned()
                .ok_or(crate::PatchError::MissingId)?,
        }),
        PatchOperation::ReplaceRepresentation { id, .. } => {
            one(PatchOperation::ReplaceRepresentation {
                id: *id,
                representation: base
                    .representations
                    .get(id)
                    .cloned()
                    .ok_or(crate::PatchError::MissingId)?,
            })
        }
        PatchOperation::SetRepresentationTarget { id, .. } => {
            one(PatchOperation::SetRepresentationTarget {
                id: *id,
                target: base
                    .representations
                    .get(id)
                    .ok_or(crate::PatchError::MissingId)?
                    .common
                    .target
                    .clone(),
            })
        }
        PatchOperation::SetColor { id, .. } => one(PatchOperation::SetColor {
            id: *id,
            color: base
                .representations
                .get(id)
                .ok_or(crate::PatchError::MissingId)?
                .common
                .color
                .clone(),
        }),
        PatchOperation::SetVisibility { id, .. } => one(PatchOperation::SetVisibility {
            id: *id,
            visible: base
                .representations
                .get(id)
                .ok_or(crate::PatchError::MissingId)?
                .common
                .visible,
        }),
        PatchOperation::SetOpacity { id, .. } => one(PatchOperation::SetOpacity {
            id: *id,
            opacity: base
                .representations
                .get(id)
                .ok_or(crate::PatchError::MissingId)?
                .opacity(),
        }),
        PatchOperation::SetVisual { id, .. } => {
            let representation = base
                .representations
                .get(id)
                .ok_or(crate::PatchError::MissingId)?;
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
            value: base
                .representations
                .get(id)
                .ok_or(crate::PatchError::MissingId)?
                .common
                .parameters
                .get(name)
                .cloned(),
        }),
        _ => None,
    })
}

fn inverse_science(
    operation: &PatchOperation,
    base: &SceneSpec,
) -> Result<Option<Vec<PatchOperation>>, crate::Error> {
    let one = |operation| Some(vec![operation]);
    Ok(match operation {
        PatchOperation::AddVolume { id, .. } => one(PatchOperation::RemoveVolume { id: *id }),
        PatchOperation::RemoveVolume { id } => one(PatchOperation::AddVolume {
            id: *id,
            volume: base
                .volumes
                .get(id)
                .cloned()
                .ok_or(crate::PatchError::MissingId)?,
        }),
        PatchOperation::AddAnnotation { id, .. } => {
            one(PatchOperation::RemoveAnnotation { id: *id })
        }
        PatchOperation::RemoveAnnotation { id } => one(PatchOperation::AddAnnotation {
            id: *id,
            annotation: base
                .annotations
                .get(id)
                .cloned()
                .ok_or(crate::PatchError::MissingId)?,
        }),
        PatchOperation::AddMeasurement { id, .. } => {
            one(PatchOperation::RemoveMeasurement { id: *id })
        }
        PatchOperation::RemoveMeasurement { id } => one(PatchOperation::AddMeasurement {
            id: *id,
            measurement: base
                .measurements
                .get(id)
                .cloned()
                .ok_or(crate::PatchError::MissingId)?,
        }),
        PatchOperation::AddScientificInteraction { id, .. } => {
            one(PatchOperation::RemoveScientificInteraction { id: *id })
        }
        PatchOperation::RemoveScientificInteraction { id } => {
            one(PatchOperation::AddScientificInteraction {
                id: *id,
                interaction: base
                    .scientific_interactions
                    .get(id)
                    .cloned()
                    .ok_or(crate::PatchError::MissingId)?,
            })
        }
        PatchOperation::AddTrajectory { id, .. } => {
            one(PatchOperation::RemoveTrajectory { id: *id })
        }
        PatchOperation::RemoveTrajectory { id } => one(PatchOperation::AddTrajectory {
            id: *id,
            trajectory: base
                .trajectories
                .get(id)
                .cloned()
                .ok_or(crate::PatchError::MissingId)?,
        }),
        _ => None,
    })
}

fn inverse_appearance(
    operation: &PatchOperation,
    base: &SceneSpec,
) -> Result<Option<Vec<PatchOperation>>, crate::Error> {
    let rule = |id| {
        base.appearance
            .get(id)
            .cloned()
            .ok_or(crate::PatchError::MissingId)
    };
    Ok(match operation {
        PatchOperation::AddAppearanceRule { id, .. } => {
            Some(vec![PatchOperation::RemoveAppearanceRule { id: *id }])
        }
        PatchOperation::ReplaceAppearanceRule { id, .. } => {
            Some(vec![PatchOperation::ReplaceAppearanceRule {
                id: *id,
                rule: rule(id)?,
            }])
        }
        PatchOperation::RemoveAppearanceRule { id } => {
            Some(vec![PatchOperation::AddAppearanceRule {
                id: *id,
                rule: rule(id)?,
            }])
        }
        _ => None,
    })
}

fn interaction<'a>(scene: &'a SceneSpec, channel: &InteractionChannel) -> Option<&'a Selection> {
    match channel {
        InteractionChannel::Selected => scene.selected.as_ref(),
        InteractionChannel::Hovered => scene.hovered.as_ref(),
        InteractionChannel::Focused => scene.focus.as_ref(),
        InteractionChannel::Muted => scene.muted.as_ref(),
        InteractionChannel::Hidden => scene.hidden.as_ref(),
        InteractionChannel::Custom(name) => scene.custom_interactions.get(name),
    }
}
