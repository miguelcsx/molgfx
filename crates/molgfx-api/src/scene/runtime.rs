//! Validation and physical resolution behind the semantic scene boundary.

use crate::error::{Error, PatchError};
use crate::id::{RepresentationId, StructureId};
use crate::scene::Resolution;
use crate::spec::{InteractionChannel, PatchOperation, ScenePatch, SceneSpec};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(crate) fn structure_hash(structure: &molgfx_core::MolecularSource) -> Box<str> {
    let mut hash = Sha256::new();
    hash.update(structure.coordinates().len().to_le_bytes());
    for coordinate in structure.coordinates() {
        for lane in coordinate {
            hash.update(lane.to_bits().to_le_bytes());
        }
    }
    for atom in structure.topology().atoms.iter() {
        hash.update([atom.element]);
        hash.update(atom.residue.to_le_bytes());
    }
    for bond in structure.topology().bonds.iter() {
        hash.update(bond.atoms[0].to_le_bytes());
        hash.update(bond.atoms[1].to_le_bytes());
        hash.update([u8::from(bond.aromatic)]);
    }
    if let Some(native) = structure.molframe() {
        for chain in native.chains() {
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
                }
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
    validate_touched_domains(&candidate, &patch.operations)?;
    advance_domain_revisions(&mut candidate, &patch.operations);
    if !patch.operations.is_empty() {
        candidate.revision = candidate.revision.wrapping_add(1);
    }
    Ok(candidate)
}

pub(crate) fn apply_operation(
    candidate: &mut SceneSpec,
    operation: &PatchOperation,
) -> Result<(), Error> {
    if crate::patch::science_ops::apply(candidate, operation)? {
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
        PatchOperation::AddRepresentation { .. }
        | PatchOperation::RemoveRepresentation { .. }
        | PatchOperation::ReplaceRepresentation { .. }
        | PatchOperation::SetVisibility { .. }
        | PatchOperation::SetOpacity { .. }
        | PatchOperation::SetVisual { .. }
        | PatchOperation::SetParameter { .. }
        | PatchOperation::AddVolume { .. }
        | PatchOperation::RemoveVolume { .. }
        | PatchOperation::AddAnnotation { .. }
        | PatchOperation::RemoveAnnotation { .. }
        | PatchOperation::AddMeasurement { .. }
        | PatchOperation::RemoveMeasurement { .. }
        | PatchOperation::AddScientificInteraction { .. }
        | PatchOperation::RemoveScientificInteraction { .. }
        | PatchOperation::AddTrajectory { .. }
        | PatchOperation::RemoveTrajectory { .. } => {
            return Err(Error::InvalidSpec(
                "scientific patch operation was not dispatched".to_owned(),
            ));
        }
    }
    Ok(())
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

fn validate_touched_domains(
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
            | PatchOperation::AddVolume { .. }
            | PatchOperation::RemoveVolume { .. }
            | PatchOperation::AddAnnotation { .. }
            | PatchOperation::RemoveAnnotation { .. }
            | PatchOperation::AddMeasurement { .. }
            | PatchOperation::RemoveMeasurement { .. }
            | PatchOperation::AddScientificInteraction { .. }
            | PatchOperation::RemoveScientificInteraction { .. }
            | PatchOperation::AddTrajectory { .. }
            | PatchOperation::RemoveTrajectory { .. }
            | PatchOperation::SetVisibility { .. }
            | PatchOperation::SetOpacity { .. }
            | PatchOperation::SetVisual { .. }
            | PatchOperation::SetParameter { .. } => {}
        }
    }
    Ok(())
}

pub(crate) fn advance_domain_revisions(candidate: &mut SceneSpec, operations: &[PatchOperation]) {
    let structural = operations.iter().any(|operation| {
        matches!(
            operation,
            PatchOperation::AddRepresentation { .. }
                | PatchOperation::RemoveRepresentation { .. }
                | PatchOperation::ReplaceRepresentation { .. }
        )
    });
    let selection = structural
        || operations
            .iter()
            .any(|operation| matches!(operation, PatchOperation::SetVisibility { .. }));
    let appearance = structural
        || operations.iter().any(|operation| {
            matches!(
                operation,
                PatchOperation::SetOpacity { .. }
                    | PatchOperation::SetVisual { .. }
                    | PatchOperation::SetParameter { .. }
            )
        });
    let interaction = operations.iter().any(|operation| {
        matches!(
            operation,
            PatchOperation::SetFocus { .. } | PatchOperation::SetInteraction { .. }
        )
    });
    let view = operations
        .iter()
        .any(|operation| matches!(operation, PatchOperation::SetCamera { .. }));
    if selection {
        candidate.revisions.selection = candidate.revisions.selection.wrapping_add(1);
    }
    if appearance {
        candidate.revisions.appearance = candidate.revisions.appearance.wrapping_add(1);
    }
    if interaction {
        candidate.revisions.interaction = candidate.revisions.interaction.wrapping_add(1);
    }
    if view {
        candidate.revisions.view = candidate.revisions.view.wrapping_add(1);
    }
    if operations.iter().any(|operation| {
        matches!(
            operation,
            PatchOperation::AddVolume { .. } | PatchOperation::RemoveVolume { .. }
        )
    }) {
        candidate.revisions.volume_data = candidate.revisions.volume_data.wrapping_add(1);
    }
    if operations.iter().any(|operation| {
        matches!(
            operation,
            PatchOperation::AddAnnotation { .. } | PatchOperation::RemoveAnnotation { .. }
        )
    }) {
        candidate.revisions.annotation = candidate.revisions.annotation.wrapping_add(1);
    }
    if operations.iter().any(|operation| {
        matches!(
            operation,
            PatchOperation::AddMeasurement { .. } | PatchOperation::RemoveMeasurement { .. }
        )
    }) {
        candidate.revisions.measurement = candidate.revisions.measurement.wrapping_add(1);
    }
    if operations.iter().any(|operation| {
        matches!(
            operation,
            PatchOperation::AddScientificInteraction { .. }
                | PatchOperation::RemoveScientificInteraction { .. }
        )
    }) {
        candidate.revisions.scientific_interaction =
            candidate.revisions.scientific_interaction.wrapping_add(1);
    }
    if operations.iter().any(|operation| {
        matches!(
            operation,
            PatchOperation::AddTrajectory { .. } | PatchOperation::RemoveTrajectory { .. }
        )
    }) {
        candidate.revisions.trajectory_data = candidate.revisions.trajectory_data.wrapping_add(1);
    }
}

pub(crate) fn resolve(
    spec: &SceneSpec,
    structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    property_bindings: &BTreeMap<Box<str>, crate::ScalarPropertyBinding>,
) -> Result<Resolution, Error> {
    let Some((_, first)) = structures.first_key_value() else {
        return Err(Error::InvalidSpec(
            "a renderable scene requires a bound structure".to_owned(),
        ));
    };
    let mut scene = molgfx_core::Scene::from_source(first.clone())?;
    for (_, structure) in structures.iter().skip(1) {
        let _ = scene.add_source(structure.clone())?;
    }
    let structure_handles = structures
        .keys()
        .copied()
        .zip(scene.structures().map(|(handle, _)| handle))
        .collect::<BTreeMap<_, _>>();
    let channels: Vec<Box<str>> = spec.custom_interactions.keys().cloned().collect();
    let properties = crate::scene::properties::resolve_bindings(
        spec,
        structures,
        property_bindings,
        &structure_handles,
        &mut scene,
    )?;
    let mut handles = BTreeMap::new();
    let mut selections = BTreeMap::new();
    let mut visuals = BTreeMap::new();
    for (id, representation) in &spec.representations {
        let structure = representation.common.structure.ok_or_else(|| {
            Error::InvalidSpec("representation has no structure target".to_owned())
        })?;
        let core_structure = structure_handles.get(&structure).copied().ok_or_else(|| {
            Error::InvalidSpec("representation structure is not bound".to_owned())
        })?;
        let selection_key = format!(
            "{}:{}",
            structure.get(),
            representation.common.target.stable_hash()?
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
        let (native, visual) = representation.native(crate::spec::lowering::Lowering {
            properties: &properties,
            channels: &channels,
        })?;
        let handle = scene.represent(selection, native)?;
        if !representation.common.visible {
            scene.hide(handle);
        }
        let _ = handles.insert(*id, handle);
        if let Some(visual) = visual {
            let _ = visuals.insert(*id, visual);
        }
    }
    crate::scene::interaction::write_states(&mut scene, spec)?;
    Ok(Resolution {
        scene,
        representations: handles,
        selections,
        visuals,
        properties,
    })
}

pub(crate) fn next_representation_id(spec: &SceneSpec) -> Result<u64, Error> {
    next_id(
        spec.representations
            .last_key_value()
            .map(|(id, _)| id.get()),
        "representation",
    )
}

pub(crate) fn next_structure_id(spec: &SceneSpec) -> Result<u64, Error> {
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

pub(crate) fn canonical_selection_count(spec: &SceneSpec) -> usize {
    spec.representations
        .values()
        .map(|representation| (representation.structure_id(), representation.selection()))
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}
