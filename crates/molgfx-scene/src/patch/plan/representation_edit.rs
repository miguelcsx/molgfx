//! A patch that only adds, removes or replaces plain representations.
//!
//! Such a patch changes which representations exist and nothing else the
//! resolution derives: structures, columns, overlays and interaction states
//! all come from parts of the specification the patch leaves alone. Resolving
//! the whole specification again would therefore rebuild every other
//! representation for nothing, so this plan lowers and installs only the
//! representations the patch names, in place.
//!
//! Anything it cannot prove local — symbols, analytical surfaces, a recoloured
//! structure with no sibling to read the overlay from — declines, and the
//! caller falls back to the full resolution, which also owns every error.

use super::PatchInputs;
use crate::PatchOperation;
use crate::error::Error;
use crate::id::{RepresentationId, StructureId};
use crate::representation::form::{RepresentationFormSpec, RepresentationSpec};
use crate::scene::apply::candidate_spec;
use crate::spec::{ScenePatch, SceneSpec};
use crate::visual::ResolvedVisual;
use molgfx_core::{
    AtomSelection, ColorOverlay, RepresentationConfig, RepresentationHandle, RepresentationTarget,
    SelectionHandle, StructureHandle,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

/// One representation, lowered and waiting to be installed.
struct Prepared {
    id: RepresentationId,
    structure: StructureHandle,
    rows: Arc<AtomSelection>,
    fingerprint: u64,
    key: String,
    config: RepresentationConfig,
    visual: Option<ResolvedVisual>,
    visible: bool,
}

/// A prepared representation-only patch.
pub(crate) struct RepresentationEdit {
    spec: SceneSpec,
    gone: Vec<(RepresentationId, RepresentationHandle)>,
    added: Vec<Prepared>,
}

/// The live scene state an edit installs into.
pub(crate) struct EditTarget<'a> {
    pub(crate) spec: &'a mut SceneSpec,
    pub(crate) scene: &'a mut molgfx_core::Scene,
    pub(crate) handles: &'a mut BTreeMap<RepresentationId, RepresentationHandle>,
    pub(crate) selections: &'a mut BTreeMap<String, SelectionHandle>,
    pub(crate) visuals: &'a mut BTreeMap<RepresentationId, ResolvedVisual>,
}

/// Whether a representation is drawn as an ordinary selection-backed record.
fn is_plain(representation: &RepresentationSpec) -> bool {
    !matches!(
        representation.form,
        RepresentationFormSpec::Snfg { .. } | RepresentationFormSpec::MolecularSurface { .. }
    )
}

fn names_representation(operation: &PatchOperation) -> Option<RepresentationId> {
    match operation {
        PatchOperation::AddRepresentation { id, .. }
        | PatchOperation::RemoveRepresentation { id }
        | PatchOperation::ReplaceRepresentation { id, .. } => Some(*id),
        _ => None,
    }
}

impl RepresentationEdit {
    /// Prepares the patch, or returns `None` when it is not a local one.
    ///
    /// # Errors
    ///
    /// Returns the candidate-specification error a malformed patch raises.
    pub(crate) fn prepare(
        inputs: PatchInputs<'_>,
        patch: &ScenePatch,
    ) -> Result<Option<Self>, Error> {
        let Some(named) = patch
            .operations
            .iter()
            .map(names_representation)
            .collect::<Option<BTreeSet<_>>>()
            .filter(|named| !named.is_empty())
        else {
            return Ok(None);
        };
        let candidate = candidate_spec(inputs.spec, patch)?;
        let mut gone = Vec::new();
        let mut incoming = Vec::new();
        for id in &named {
            if let Some(previous) = inputs.spec.representations.get(id) {
                let handle = inputs.handles.get(id).copied();
                let (true, Some(handle)) = (is_plain(previous), handle) else {
                    return Ok(None);
                };
                gone.push((*id, handle));
            }
            if let Some(next) = candidate.representations.get(id) {
                if !is_plain(next) {
                    return Ok(None);
                }
                incoming.push((*id, next));
            }
        }
        let Some(overlays) = overlays_of(inputs, &candidate, &named, &incoming) else {
            return Ok(None);
        };
        let structure_handles = inputs.structure_handles();
        let channels: Vec<Box<str>> = candidate.custom_interactions.keys().cloned().collect();
        let mut added = Vec::with_capacity(incoming.len());
        for (id, representation) in incoming {
            let Some(structure) = representation.common.structure else {
                return Ok(None);
            };
            let (Some(source), Some(core)) = (
                inputs.structures.get(&structure),
                structure_handles.get(&structure),
            ) else {
                return Ok(None);
            };
            let (config, visual) = representation.native(crate::spec::lowering::Lowering {
                properties: inputs.properties,
                surfaces: Some((inputs.overlay_bindings, inputs.structures)),
                channels: &channels,
            })?;
            let overlay = overlays.get(&structure).copied().flatten();
            let target = &representation.common.target;
            let (rows, fingerprint) = inputs.rows.rows(structure, source, target)?;
            added.push(Prepared {
                id,
                structure: *core,
                rows,
                fingerprint,
                key: format!("{}:{}", structure.get(), target.stable_hash()?),
                config: config.color_overlay(overlay),
                visual,
                visible: representation.common.visible,
            });
        }
        Ok(Some(Self {
            spec: candidate,
            gone,
            added,
        }))
    }

    /// The specification the edit installs.
    pub(crate) fn spec(&self) -> &SceneSpec {
        &self.spec
    }

    /// Installs every prepared representation, then retires the replaced ones.
    ///
    /// Additions come first and are undone together if one fails, so the scene
    /// is unchanged on error; removals cannot fail.
    ///
    /// # Errors
    ///
    /// Returns the core error of the first representation that did not lower.
    pub(crate) fn commit(self, target: EditTarget<'_>) -> Result<(), Error> {
        let EditTarget {
            spec,
            scene,
            handles,
            selections,
            visuals,
        } = target;
        let mut made_selections: Vec<(String, SelectionHandle)> = Vec::new();
        let mut made: Vec<(
            RepresentationId,
            RepresentationHandle,
            Option<ResolvedVisual>,
        )> = Vec::with_capacity(self.added.len());
        for prepared in self.added {
            let shared = selections
                .get(&prepared.key)
                .copied()
                .filter(|handle| scene.selection(*handle).is_some());
            let selection = match shared {
                Some(shared) => Ok(shared),
                None => scene
                    .add_query_selection(
                        prepared.structure,
                        (*prepared.rows).clone(),
                        prepared.fingerprint,
                    )
                    .inspect(|created| {
                        made_selections.push((prepared.key.clone(), *created));
                        let _ = selections.insert(prepared.key.clone(), *created);
                    }),
            };
            let drawn = selection.and_then(|selection| scene.represent(selection, prepared.config));
            match drawn {
                Ok(handle) => {
                    if !prepared.visible {
                        scene.hide(handle);
                    }
                    made.push((prepared.id, handle, prepared.visual));
                }
                Err(error) => {
                    for (_, handle, _) in made {
                        scene.remove_representation(handle);
                    }
                    for (key, handle) in made_selections {
                        let _ = scene.remove_selection(handle);
                        let _ = selections.remove(&key);
                    }
                    return Err(error.into());
                }
            }
        }
        let mut released: HashSet<SelectionHandle> = HashSet::new();
        for (id, handle) in self.gone {
            if let Some(RepresentationTarget::Selection(selection)) =
                scene.representation(handle).map(|stored| stored.target)
            {
                let _ = released.insert(selection);
            }
            scene.remove_representation(handle);
            let _ = handles.remove(&id);
            let _ = visuals.remove(&id);
        }
        for (id, handle, visual) in made {
            let _ = handles.insert(id, handle);
            if let Some(visual) = visual {
                let _ = visuals.insert(id, visual);
            }
        }
        released.retain(|selection| scene.remove_selection(*selection).is_ok());
        if !released.is_empty() {
            selections.retain(|_, handle| !released.contains(handle));
        }
        *spec = self.spec;
        Ok(())
    }
}

/// The colour overlay each incoming representation's structure carries.
///
/// A structure with appearance rules shares one overlay across all of its
/// representations; the edit reads it from a sibling it does not touch, and
/// declines when no sibling exists to read it from.
fn overlays_of(
    inputs: PatchInputs<'_>,
    candidate: &SceneSpec,
    named: &BTreeSet<RepresentationId>,
    incoming: &[(RepresentationId, &RepresentationSpec)],
) -> Option<BTreeMap<StructureId, Option<ColorOverlay>>> {
    let ruled = crate::scene::appearance::ruled_structures(candidate);
    let mut overlays: BTreeMap<StructureId, Option<ColorOverlay>> = BTreeMap::new();
    let mut unresolved: BTreeSet<StructureId> = BTreeSet::new();
    for (_, representation) in incoming {
        let structure = representation.common.structure?;
        if ruled.contains(&structure) {
            let _ = unresolved.insert(structure);
        } else {
            let _ = overlays.insert(structure, None);
        }
    }
    if unresolved.is_empty() {
        return Some(overlays);
    }
    for (id, representation) in &inputs.spec.representations {
        let Some(structure) = representation.common.structure else {
            continue;
        };
        if named.contains(id) || !unresolved.contains(&structure) {
            continue;
        }
        let Some(handle) = inputs.handles.get(id) else {
            continue;
        };
        let overlay = inputs.scene.representation(*handle)?.color_overlay;
        let _ = overlays.insert(structure, overlay);
        let _ = unresolved.remove(&structure);
        if unresolved.is_empty() {
            return Some(overlays);
        }
    }
    None
}
