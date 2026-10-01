//! Applying one patch operation to a candidate specification, and
//! validating the domains it touched.

use crate::error::{Error, PatchError};
use crate::id::RepresentationId;
use crate::spec::{InteractionChannel, PatchOperation, ScenePatch, SceneSpec, StructureSource};
use std::collections::BTreeMap;

pub(crate) fn candidate_spec(current: &SceneSpec, patch: &ScenePatch) -> Result<SceneSpec, Error> {
    let mut candidate = current.clone();
    for operation in &patch.operations {
        apply_operation(&mut candidate, operation)?;
    }
    validate_touched_domains(&candidate, &patch.operations)?;
    if !patch.operations.is_empty() {
        candidate.revision = candidate.revision.wrapping_add(1);
    }
    Ok(candidate)
}

pub(crate) fn apply_operation(
    candidate: &mut SceneSpec,
    operation: &PatchOperation,
) -> Result<(), Error> {
    if crate::patch::overlay_ops::apply(candidate, operation)? {
        return Ok(());
    }
    if crate::patch::appearance_ops::apply(candidate, operation)? {
        return Ok(());
    }
    if apply_domain_operation(candidate, operation)? {
        return Ok(());
    }
    if apply_representation_operation(candidate, operation)? {
        return Ok(());
    }
    match operation {
        PatchOperation::SetFocus { selection } => candidate.focus.clone_from(selection),
        PatchOperation::SetInteraction { channel, selection } => match channel {
            InteractionChannel::Selected => candidate.selected.clone_from(selection),
            InteractionChannel::Hovered => candidate.hovered.clone_from(selection),
            InteractionChannel::Focused => candidate.focus.clone_from(selection),
            InteractionChannel::Muted => candidate.muted.clone_from(selection),
            InteractionChannel::Hidden => candidate.hidden.clone_from(selection),
            InteractionChannel::Custom(name) => {
                if let Some(selection) = selection {
                    let _ = candidate
                        .custom_interactions
                        .insert(name.clone(), selection.clone());
                } else {
                    let _ = candidate.custom_interactions.remove(name);
                }
            }
        },
        PatchOperation::SetCamera { camera } => candidate.camera = *camera,
        PatchOperation::AddStructure { id, source } => {
            let descriptor = StructureSource {
                content_hash: source.content_hash.clone(),
                uri: None,
                format: None,
            };
            insert_unique(&mut candidate.structures, *id, descriptor, "structure")?;
        }
        PatchOperation::AddRepresentation { .. }
        | PatchOperation::RemoveRepresentation { .. }
        | PatchOperation::ReplaceRepresentation { .. }
        | PatchOperation::SetRepresentationTarget { .. }
        | PatchOperation::SetColor { .. }
        | PatchOperation::AddAppearanceRule { .. }
        | PatchOperation::ReplaceAppearanceRule { .. }
        | PatchOperation::RemoveAppearanceRule { .. }
        | PatchOperation::SetVisibility { .. }
        | PatchOperation::SetOpacity { .. }
        | PatchOperation::SetVisual { .. }
        | PatchOperation::SetParameter { .. }
        | PatchOperation::AddVolume { .. }
        | PatchOperation::RemoveVolume { .. }
        | PatchOperation::SetVolumeIsovalue { .. }
        | PatchOperation::AddAnnotation { .. }
        | PatchOperation::RemoveAnnotation { .. }
        | PatchOperation::RemoveMeasurement { .. }
        | PatchOperation::AddMeasurement { .. }
        | PatchOperation::AddInteraction { .. }
        | PatchOperation::RemoveInteraction { .. }
        | PatchOperation::AddTrajectory { .. }
        | PatchOperation::AddEllipsoids { .. }
        | PatchOperation::RemoveEllipsoids { .. }
        | PatchOperation::AddPlane { .. }
        | PatchOperation::RemovePlane { .. }
        | PatchOperation::SetAssembly { .. }
        | PatchOperation::SetFitting { .. }
        | PatchOperation::SetValidation { .. }
        | PatchOperation::SetMovieExport { .. }
        | PatchOperation::SetSnapshot { .. }
        | PatchOperation::RemoveTrajectory { .. } => {
            return Err(Error::InvalidSpec(
                "overlay patch operation was not dispatched".to_owned(),
            ));
        }
    }
    Ok(())
}

fn apply_domain_operation(
    candidate: &mut SceneSpec,
    operation: &PatchOperation,
) -> Result<bool, Error> {
    if let PatchOperation::SetAssembly { assembly } = operation {
        if let Some(value) = assembly {
            value.validate()?;
        }
        candidate.assembly.clone_from(assembly);
        return Ok(true);
    }
    let (key, value) = match operation {
        PatchOperation::SetFitting { fitting } => {
            if let Some(value) = fitting {
                value.validate()?;
                (
                    "molgfx.fitting",
                    Some(
                        serde_json::to_value(value)
                            .map_err(|error| Error::InvalidSpec(error.to_string()))?,
                    ),
                )
            } else {
                ("molgfx.fitting", None)
            }
        }
        PatchOperation::SetValidation { findings } => {
            for finding in findings {
                finding.validate()?;
            }
            (
                "molgfx.validation",
                Some(
                    serde_json::to_value(findings)
                        .map_err(|error| Error::InvalidSpec(error.to_string()))?,
                ),
            )
        }
        PatchOperation::SetMovieExport { request } => {
            if let Some(value) = request {
                value.validate()?;
                (
                    "molgfx.movie_export",
                    Some(
                        serde_json::to_value(value)
                            .map_err(|error| Error::InvalidSpec(error.to_string()))?,
                    ),
                )
            } else {
                ("molgfx.movie_export", None)
            }
        }
        PatchOperation::SetSnapshot { snapshot } => {
            if let Some(value) = &**snapshot {
                value.clone().restore().map_err(Error::InvalidSpec)?;
                (
                    "molgfx.snapshot",
                    Some(
                        serde_json::to_value(value)
                            .map_err(|error| Error::InvalidSpec(error.to_string()))?,
                    ),
                )
            } else {
                ("molgfx.snapshot", None)
            }
        }
        _ => return Ok(false),
    };
    if let Some(value) = value {
        candidate.extensions.insert(key.into(), value);
    } else {
        candidate.extensions.remove(key);
    }
    Ok(true)
}
fn apply_representation_operation(
    candidate: &mut SceneSpec,
    operation: &PatchOperation,
) -> Result<bool, Error> {
    match operation {
        PatchOperation::AddRepresentation { id, representation } => {
            representation.validate()?;
            insert_unique(
                &mut candidate.representations,
                *id,
                representation.clone(),
                "representation",
            )?;
        }
        PatchOperation::RemoveRepresentation { id } => {
            remove_existing(&mut candidate.representations, id)?;
        }
        PatchOperation::ReplaceRepresentation {
            id,
            representation: value,
        } => {
            value.validate()?;
            *representation_mut(candidate, *id)? = value.clone();
        }
        PatchOperation::SetRepresentationTarget { id, target } => {
            let _ = target.fingerprint()?;
            representation_mut(candidate, *id)?.common.target = target.clone();
        }
        PatchOperation::SetColor { id, color } => {
            color.validate()?;
            representation_mut(candidate, *id)?.common.color = color.clone();
        }
        PatchOperation::SetVisibility { id, visible } => {
            representation_mut(candidate, *id)?.common.visible = *visible;
        }
        PatchOperation::SetOpacity { id, opacity } => {
            if !opacity.is_finite() || !(0.0..=1.0).contains(opacity) {
                return Err(PatchError::Invalid(
                    "opacity must be finite and between zero and one".to_owned(),
                )
                .into());
            }
            representation_mut(candidate, *id)?.common.opacity = *opacity;
        }
        PatchOperation::SetVisual { id, visual } => {
            let representation = representation_mut(candidate, *id)?;
            representation.common.visual.clone_from(visual);
            representation.common.parameters.clear();
            representation.validate()?;
        }
        PatchOperation::SetParameter { id, name, value } => {
            let representation = representation_mut(candidate, *id)?;
            if let Some(value) = value {
                let _ = representation
                    .common
                    .parameters
                    .insert(name.clone(), value.clone());
            } else {
                let _ = representation.common.parameters.remove(name);
            }
            representation.validate()?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn representation_mut(
    candidate: &mut SceneSpec,
    id: RepresentationId,
) -> Result<&mut crate::RepresentationSpec, Error> {
    candidate
        .representations
        .get_mut(&id)
        .ok_or_else(|| Error::from(PatchError::MissingId))
}

pub(crate) fn insert_unique<K: Ord, V>(
    values: &mut BTreeMap<K, V>,
    id: K,
    value: V,
    kind: &str,
) -> Result<(), Error> {
    if values.insert(id, value).is_some() {
        return Err(PatchError::Invalid(format!("{kind} ID already exists")).into());
    }
    Ok(())
}

pub(crate) fn remove_existing<K: Ord, V>(values: &mut BTreeMap<K, V>, id: &K) -> Result<(), Error> {
    if values.remove(id).is_none() {
        return Err(PatchError::MissingId.into());
    }
    Ok(())
}

pub(crate) fn validate_touched_domains(
    candidate: &SceneSpec,
    operations: &[PatchOperation],
) -> Result<(), Error> {
    for operation in operations {
        match operation {
            PatchOperation::AddRepresentation { representation, .. }
            | PatchOperation::ReplaceRepresentation { representation, .. } => {
                let Some(structure) = representation.common.structure else {
                    return Err(Error::InvalidSpec(
                        "every representation must target a structure".to_owned(),
                    ));
                };
                if !candidate.structures.contains_key(&structure) {
                    return Err(Error::InvalidSpec(
                        "representation targets an unknown structure".to_owned(),
                    ));
                }
                let _ = representation.common.target.fingerprint()?;
            }
            PatchOperation::SetFocus { selection }
            | PatchOperation::SetInteraction { selection, .. } => {
                if let Some(selection) = selection {
                    let _ = selection.fingerprint()?;
                }
            }
            PatchOperation::SetCamera { .. } => candidate.validate_camera()?,
            PatchOperation::RemoveRepresentation { .. }
            | PatchOperation::AddStructure { .. }
            | PatchOperation::AddVolume { .. }
            | PatchOperation::RemoveVolume { .. }
            | PatchOperation::SetVolumeIsovalue { .. }
            | PatchOperation::AddAnnotation { .. }
            | PatchOperation::RemoveAnnotation { .. }
            | PatchOperation::RemoveMeasurement { .. }
            | PatchOperation::AddMeasurement { .. }
            | PatchOperation::AddInteraction { .. }
            | PatchOperation::RemoveInteraction { .. }
            | PatchOperation::AddTrajectory { .. }
            | PatchOperation::RemoveTrajectory { .. }
            | PatchOperation::AddEllipsoids { .. }
            | PatchOperation::RemoveEllipsoids { .. }
            | PatchOperation::AddPlane { .. }
            | PatchOperation::RemovePlane { .. }
            | PatchOperation::SetVisibility { .. }
            | PatchOperation::SetOpacity { .. }
            | PatchOperation::SetVisual { .. }
            | PatchOperation::SetParameter { .. }
            | PatchOperation::SetRepresentationTarget { .. }
            | PatchOperation::SetColor { .. }
            | PatchOperation::AddAppearanceRule { .. }
            | PatchOperation::ReplaceAppearanceRule { .. }
            | PatchOperation::RemoveAppearanceRule { .. }
            | PatchOperation::SetAssembly { .. }
            | PatchOperation::SetFitting { .. }
            | PatchOperation::SetValidation { .. }
            | PatchOperation::SetMovieExport { .. }
            | PatchOperation::SetSnapshot { .. } => {}
        }
    }
    Ok(())
}
