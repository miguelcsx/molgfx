//! Validation and physical resolution behind the semantic scene boundary.

use crate::error::{Error, PatchError};
use crate::id::{RepresentationId, StructureId};
use crate::scene::Resolution;
use crate::spec::{InteractionChannel, PatchOperation, ScenePatch, SceneSpec};
use molgfx_core::RepresentationHandle;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(super) fn structure_hash(structure: &molframe::Structure) -> Box<str> {
    let mut hash = Sha256::new();
    hash.update(structure.atom_count().to_le_bytes());
    hash.update(structure.chain_count().to_le_bytes());
    hash.update(structure.residue_count().to_le_bytes());
    hash.update(structure.model_count().to_le_bytes());
    for model in 0..structure.model_count() {
        let Ok(model) = u32::try_from(model) else {
            break;
        };
        if let Some(coordinates) = structure.model_coordinates(molframe::ModelIndex::new(model)) {
            for coordinate in coordinates {
                for lane in coordinate {
                    hash.update(lane.to_bits().to_le_bytes());
                }
            }
        }
    }
    for chain in structure.chains() {
        hash_text(&mut hash, chain.label());
        hash_text(&mut hash, chain.auth_label());
        for residue in chain.residues() {
            hash_text(&mut hash, residue.name());
            hash_text(&mut hash, residue.auth_name());
            hash_text(&mut hash, residue.ins_code());
            hash.update(
                residue
                    .label_seq_id()
                    .into_iter()
                    .fold(i32::MIN, |_, value| value)
                    .to_le_bytes(),
            );
            hash.update(
                residue
                    .auth_seq_id()
                    .into_iter()
                    .fold(i32::MIN, |_, value| value)
                    .to_le_bytes(),
            );
            hash.update([u8::from(residue.is_het())]);
            for atom in residue.atoms() {
                hash_text(&mut hash, atom.name());
                hash_text(&mut hash, atom.auth_name());
                hash_text(&mut hash, atom.alt_label());
                hash.update([atom.element().map_or(0, molframe::Element::atomic_number)]);
                hash.update([atom
                    .formal_charge()
                    .map_or(u8::MAX, |value| value.to_le_bytes()[0])]);
            }
        }
    }
    format!("{digest:x}", digest = hash.finalize()).into_boxed_str()
}

fn hash_text(hash: &mut Sha256, value: Option<&str>) {
    let value = value.map_or(&[][..], str::as_bytes);
    hash.update(value.len().to_le_bytes());
    hash.update(value);
}

pub(crate) fn candidate_spec(current: &SceneSpec, patch: &ScenePatch) -> Result<SceneSpec, Error> {
    let mut candidate = current.clone();
    for operation in &patch.operations {
        apply_operation(&mut candidate, operation)?;
    }
    candidate.validate_selections()?;
    candidate.validate_camera()?;
    for operation in &patch.operations {
        advance_domain_revision(&mut candidate, operation);
    }
    if !patch.operations.is_empty() {
        candidate.revision = candidate.revision.wrapping_add(1);
    }
    Ok(candidate)
}

pub(crate) fn apply_operation(
    candidate: &mut SceneSpec,
    operation: &PatchOperation,
) -> Result<(), Error> {
    match operation {
        PatchOperation::AddRepresentation { id, representation } => {
            representation.validate()?;
            if candidate.representations.contains_key(id) {
                return Err(
                    PatchError::Invalid("representation ID already exists".to_owned()).into(),
                );
            }
            let _ = candidate
                .representations
                .insert(*id, representation.clone());
        }
        PatchOperation::RemoveRepresentation { id } => {
            if candidate.representations.remove(id).is_none() {
                return Err(PatchError::MissingId.into());
            }
        }
        PatchOperation::ReplaceRepresentation { id, representation } => {
            representation.validate()?;
            let Some(current) = candidate.representations.get_mut(id) else {
                return Err(PatchError::MissingId.into());
            };
            *current = representation.clone();
        }
        PatchOperation::SetVisibility { id, visible } => {
            let Some(representation) = candidate.representations.get_mut(id) else {
                return Err(PatchError::MissingId.into());
            };
            representation.visible = *visible;
        }
        PatchOperation::SetOpacity { id, opacity } => {
            if !opacity.is_finite() || !(0.0..=1.0).contains(opacity) {
                return Err(PatchError::Invalid(
                    "opacity must be finite and between zero and one".to_owned(),
                )
                .into());
            }
            let Some(representation) = candidate.representations.get_mut(id) else {
                return Err(PatchError::MissingId.into());
            };
            representation.opacity = *opacity;
        }
        PatchOperation::SetVisual { id, visual } => {
            if let Some(visual) = visual {
                let _ = visual.compile()?;
            }
            let Some(representation) = candidate.representations.get_mut(id) else {
                return Err(PatchError::MissingId.into());
            };
            representation.visual.clone_from(visual);
            representation.parameters.clear();
        }
        PatchOperation::SetParameter { id, name, value } => {
            let Some(representation) = candidate.representations.get_mut(id) else {
                return Err(PatchError::MissingId.into());
            };
            match value {
                Some(value) => {
                    let _ = representation
                        .parameters
                        .insert(name.clone(), value.clone());
                }
                None => {
                    let _ = representation.parameters.remove(name);
                }
            }
            representation.validate()?;
        }
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
    }
    Ok(())
}

fn advance_domain_revision(candidate: &mut SceneSpec, operation: &PatchOperation) {
    match operation {
        PatchOperation::AddRepresentation { .. }
        | PatchOperation::RemoveRepresentation { .. }
        | PatchOperation::ReplaceRepresentation { .. } => {
            candidate.revisions.selection = candidate.revisions.selection.wrapping_add(1);
            candidate.revisions.appearance = candidate.revisions.appearance.wrapping_add(1);
        }
        PatchOperation::SetOpacity { .. }
        | PatchOperation::SetVisual { .. }
        | PatchOperation::SetParameter { .. } => {
            candidate.revisions.appearance = candidate.revisions.appearance.wrapping_add(1);
        }
        PatchOperation::SetVisibility { .. } => {
            candidate.revisions.selection = candidate.revisions.selection.wrapping_add(1);
        }
        PatchOperation::SetFocus { .. } | PatchOperation::SetInteraction { .. } => {
            candidate.revisions.interaction = candidate.revisions.interaction.wrapping_add(1);
        }
        PatchOperation::SetCamera { .. } => {
            candidate.revisions.view = candidate.revisions.view.wrapping_add(1);
        }
    }
}

pub(super) fn resolve(
    spec: &SceneSpec,
    structures: &BTreeMap<StructureId, molframe::Structure>,
) -> Result<Resolution, Error> {
    let Some((_, first)) = structures.first_key_value() else {
        return Err(Error::InvalidSpec(
            "a renderable scene requires a bound structure".to_owned(),
        ));
    };
    let mut scene = molgfx_core::Scene::from_structure(first)?;
    for (_, structure) in structures.iter().skip(1) {
        let _ = scene.add_structure(structure)?;
    }
    let structure_handles = structures
        .keys()
        .copied()
        .zip(scene.structures().map(|(handle, _)| handle))
        .collect::<BTreeMap<_, _>>();
    let mut handles = BTreeMap::new();
    let mut selections = BTreeMap::new();
    for (id, representation) in &spec.representations {
        let structure = representation.structure.ok_or_else(|| {
            Error::InvalidSpec("representation has no structure target".to_owned())
        })?;
        let core_structure = structure_handles.get(&structure).copied().ok_or_else(|| {
            Error::InvalidSpec("representation structure is not bound".to_owned())
        })?;
        let selection_key = format!(
            "{}:{}",
            structure.get(),
            representation.target.stable_hash()?
        );
        let selection = if let Some(selection) = selections.get(&selection_key).copied() {
            selection
        } else {
            let all_structures = scene.select_str(representation.selection())?;
            let rows = scene
                .selection_for(all_structures, core_structure)
                .cloned()
                .ok_or_else(|| Error::InvalidSpec("selection did not resolve".to_owned()))?;
            let selection = scene.add_structure_selection(core_structure, rows)?;
            let _ = selections.insert(selection_key, selection);
            selection
        };
        let handle = scene.represent(selection, representation.native()?)?;
        if !representation.visible {
            scene.hide(handle);
        }
        let _ = handles.insert(*id, handle);
    }
    Ok(Resolution {
        scene,
        representations: handles,
        selections,
    })
}

pub(super) fn apply_runtime_edits(
    scene: &mut molgfx_core::Scene,
    handles: &BTreeMap<RepresentationId, RepresentationHandle>,
    candidate: &SceneSpec,
    operations: &[PatchOperation],
) -> Result<(), Error> {
    for operation in operations {
        let id = match operation {
            PatchOperation::SetVisibility { id, .. }
            | PatchOperation::SetOpacity { id, .. }
            | PatchOperation::SetVisual { id, .. }
            | PatchOperation::SetParameter { id, .. } => Some(*id),
            _ => None,
        };
        if id.is_some_and(|id| !handles.contains_key(&id)) {
            return Err(Error::MissingId);
        }
        if let PatchOperation::SetVisual { id, .. } | PatchOperation::SetParameter { id, .. } =
            operation
        {
            let Some(source) = candidate.representations.get(id) else {
                return Err(Error::MissingId);
            };
            let Some(handle) = handles.get(id).copied() else {
                return Err(Error::MissingId);
            };
            let Some(mut representation) = scene.representation(handle).cloned() else {
                return Err(Error::MissingId);
            };
            source.apply_appearance(&mut representation)?;
        }
    }
    for operation in operations {
        match operation {
            PatchOperation::SetVisibility { id, visible } => {
                let Some(handle) = handles.get(id).copied() else {
                    return Err(Error::MissingId);
                };
                if *visible {
                    scene.show(handle);
                } else {
                    scene.hide(handle);
                }
            }
            PatchOperation::SetOpacity { id, .. } => {
                let Some(handle) = handles.get(id).copied() else {
                    return Err(Error::MissingId);
                };
                let Some(source) = candidate.representations.get(id) else {
                    return Err(Error::MissingId);
                };
                let Some(representation) = scene.representation_mut(handle) else {
                    return Err(Error::MissingId);
                };
                representation.material.opacity = source.opacity;
            }
            PatchOperation::SetVisual { id, .. } | PatchOperation::SetParameter { id, .. } => {
                let Some(handle) = handles.get(id).copied() else {
                    return Err(Error::MissingId);
                };
                let Some(source) = candidate.representations.get(id) else {
                    return Err(Error::MissingId);
                };
                let Some(representation) = scene.representation_mut(handle) else {
                    return Err(Error::MissingId);
                };
                source.apply_appearance(representation)?;
            }
            PatchOperation::SetFocus { .. }
            | PatchOperation::SetInteraction { .. }
            | PatchOperation::SetCamera { .. } => {}
            PatchOperation::AddRepresentation { .. }
            | PatchOperation::RemoveRepresentation { .. }
            | PatchOperation::ReplaceRepresentation { .. } => {
                return Err(Error::InvalidSpec(
                    "structural patch reached the incremental update path".to_owned(),
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn next_representation_id(spec: &SceneSpec) -> Result<u64, Error> {
    next_id(
        spec.representations
            .last_key_value()
            .map(|(id, _)| id.get()),
        "representation",
    )
}

#[cfg(test)]
#[path = "scene_runtime_tests.rs"]
mod tests;

pub(super) fn next_structure_id(spec: &SceneSpec) -> Result<u64, Error> {
    next_id(
        spec.structures.last_key_value().map(|(id, _)| id.get()),
        "structure",
    )
}

fn next_id(last: Option<u64>, kind: &str) -> Result<u64, Error> {
    crate::fallback(last, 0)
        .checked_add(1)
        .ok_or_else(|| Error::InvalidSpec(format!("{kind} identity space is exhausted")))
}

pub(super) fn canonical_selection_count(spec: &SceneSpec) -> usize {
    spec.representations
        .values()
        .map(|representation| (representation.structure_id(), representation.selection()))
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}
