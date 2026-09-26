//! Validation and physical resolution behind the semantic scene boundary.

use crate::error::{Error, PatchError};
use crate::id::{RepresentationId, StructureId};
use crate::scene::Resolution;
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
    if crate::patch::science_ops::apply(candidate, operation)? {
        return Ok(());
    }
    if crate::patch::appearance_ops::apply(candidate, operation)? {
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
            | PatchOperation::SetParameter { .. }
            | PatchOperation::SetRepresentationTarget { .. }
            | PatchOperation::SetColor { .. }
            | PatchOperation::AddAppearanceRule { .. }
            | PatchOperation::ReplaceAppearanceRule { .. }
            | PatchOperation::RemoveAppearanceRule { .. } => {}
        }
    }
    Ok(())
}

/// Identity of the structures a resolution is built over.
///
/// Two calls with equal keys can reuse one scene's structure assets, which is
/// what lets a representation edit skip re-materialising every atom table. A
/// source's identity and coordinate revision change when its molecule does, and
/// the structure list changes when one is added or removed, so the two
/// together are a complete key: nothing else in a specification describes a
/// structure.
pub(crate) type StructureKey = Vec<(StructureId, u64, u64)>;

/// The key for one specification's structure set.
pub(crate) fn structure_key(
    structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
) -> StructureKey {
    structures
        .iter()
        .map(|(id, source)| (*id, source.identity(), source.coordinate_revision()))
        .collect()
}

/// Structure assets a resolution can hand to the next one.
///
/// The assets are keyed by the sources they were built from, so a caller that
/// recognises the same molecule does not pay to build its atom tables twice.
#[derive(Clone, Debug, Default)]
pub(crate) struct StructureAssets {
    key: StructureKey,
    assets: Vec<molgfx_core::StructureAsset>,
}

impl StructureAssets {
    /// Records the assets of one resolved scene.
    pub(crate) fn capture(
        structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
        scene: &molgfx_core::Scene,
    ) -> Self {
        Self {
            key: structure_key(structures),
            assets: scene
                .structures()
                .map(|(_, placed)| placed.asset().clone())
                .collect(),
        }
    }

    /// The assets to reuse, when they were built from this structure set.
    fn matches(
        &self,
        structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    ) -> Option<&[molgfx_core::StructureAsset]> {
        (self.key == structure_key(structures)).then_some(self.assets.as_slice())
    }
}

pub(crate) fn resolve(
    spec: &SceneSpec,
    structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    property_bindings: &BTreeMap<Box<str>, crate::ScalarPropertyBinding>,
    science_bindings: &crate::science::ScienceBindings,
    rows: &crate::scene::selection_rows::SelectionRows,
) -> Result<Resolution, Error> {
    resolve_reusing(
        spec,
        structures,
        property_bindings,
        science_bindings,
        rows,
        None,
    )
}

/// Resolves a specification, reusing earlier structure assets when they match.
///
/// A patch that touches only representations still resolves through this path,
/// because selection identity, lowering and the interaction columns all derive
/// from the specification. What such a patch does not need is the molecules:
/// an atom table is a function of the structure alone, so handing back the
/// assets built for the same sources replaces a full per-atom
/// re-materialisation with one clone of a shared handle. The placed structures
/// are created afresh, so nothing that a caller may have made placement-local
/// is carried across.
///
/// # Errors
///
/// Returns an invalid-specification error when no structure is bound or a
/// representation targets an unbound structure, and propagates every lowering
/// failure.
pub(crate) fn resolve_reusing(
    spec: &SceneSpec,
    structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    property_bindings: &BTreeMap<Box<str>, crate::ScalarPropertyBinding>,
    science_bindings: &crate::science::ScienceBindings,
    rows: &crate::scene::selection_rows::SelectionRows,
    reusable: Option<&StructureAssets>,
) -> Result<Resolution, Error> {
    let Some((_, first)) = structures.first_key_value() else {
        return Err(Error::InvalidSpec(
            "a renderable scene requires a bound structure".to_owned(),
        ));
    };
    let mut scene = base_scene(first, structures, reusable)?;
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
    let prepared = crate::scene::appearance::prepare(
        &spec.appearance,
        &crate::scene::appearance::ruled_structures(spec),
        structures,
        &structure_handles,
        rows,
    )?;
    let mut appearance = BTreeMap::new();
    let installed = crate::scene::appearance::install(&mut scene, &mut appearance, prepared)?;
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
            // Only this representation's structure is evaluated, and an
            // unchanged query over an unchanged molecule comes from the cache.
            let source = structures.get(&structure).ok_or_else(|| {
                Error::InvalidSpec("representation structure is not bound".to_owned())
            })?;
            let (rows, fingerprint) =
                rows.rows(structure, source, &representation.common.target)?;
            let selection =
                scene.add_query_selection(core_structure, (*rows).clone(), fingerprint)?;
            let _ = selections.insert(selection_key, selection);
            selection
        };
        let (native, visual) = representation.native(crate::spec::lowering::Lowering {
            properties: &properties,
            channels: &channels,
        })?;
        let overlay = installed.overlays.get(&structure).copied().flatten();
        let native = native.color_overlay(overlay);
        let handle = scene.represent(selection, native)?;
        if !representation.common.visible {
            scene.hide(handle);
        }
        let _ = handles.insert(*id, handle);
        if let Some(visual) = visual {
            let _ = visuals.insert(*id, visual);
        }
    }
    let mut science_lowering = crate::science::lower::SciLowering {
        scene: &mut scene,
        structures,
        handles: &structure_handles,
        selections: &mut selections,
        bindings: science_bindings,
    };
    let science = crate::science::lower::lower(spec, &mut science_lowering)?;
    crate::scene::interaction::write_states(&mut scene, spec)?;
    Ok(Resolution {
        scene,
        representations: handles,
        selections,
        visuals,
        properties,
        science,
        appearance,
    })
}

/// A core scene holding every bound structure, built from earlier assets
/// when they were made from the same structure set.
fn base_scene(
    first: &molgfx_core::MolecularSource,
    structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    reusable: Option<&StructureAssets>,
) -> Result<molgfx_core::Scene, Error> {
    let reused = reusable.and_then(|assets| assets.matches(structures));
    Ok(match reused {
        // Every asset's atom table, hierarchy and source are already built.
        Some([]) => molgfx_core::Scene::new(),
        Some(assets) => {
            let mut scene = molgfx_core::Scene::new();
            for asset in assets {
                let _ = scene.add_asset(asset);
            }
            scene
        }
        None => {
            let mut scene = molgfx_core::Scene::from_source(first.clone())?;
            for (_, structure) in structures.iter().skip(1) {
                let _ = scene.add_source(structure.clone())?;
            }
            scene
        }
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
