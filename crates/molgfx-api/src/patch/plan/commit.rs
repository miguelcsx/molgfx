//! Installing a prepared local patch.
//!
//! Everything fallible and expensive happened while the plan was prepared;
//! commit writes prepared values. Physical representations are replaced in
//! one batch, so the renderer sees one consistent change, and selections and
//! class columns nothing reads any more are released afterwards.

use super::appearance::AppearanceUpdates;
use super::{LocalPatchPlan, assign};
use crate::error::Error;
use crate::id::{RepresentationId, StructureId};
use crate::scene::appearance::AppearanceColumns;
use crate::spec::SceneSpec;
use molgfx_core::{RepresentationHandle, RepresentationTarget, SelectionHandle};
use std::collections::BTreeMap;

/// The scene state a local patch mutates.
pub(crate) struct CommitTarget<'a> {
    pub(crate) spec: &'a mut SceneSpec,
    pub(crate) scene: &'a mut molgfx_core::Scene,
    pub(crate) visuals: &'a mut BTreeMap<RepresentationId, crate::visual::ResolvedVisual>,
    pub(crate) selections: &'a mut BTreeMap<String, SelectionHandle>,
    pub(crate) appearance: &'a mut AppearanceColumns,
    pub(crate) handles: &'a BTreeMap<RepresentationId, RepresentationHandle>,
}

impl LocalPatchPlan {
    pub(crate) fn commit(self, target: CommitTarget<'_>) -> Result<(), Error> {
        let CommitTarget {
            spec,
            scene,
            visuals,
            selections,
            appearance,
            handles,
        } = target;
        // Channel queries were already validated during preparation, so this
        // evaluates known-good queries against the scene that owns the columns.
        if self.interactions.touched() {
            // Only the channels this patch actually re-stated need writing. The
            // dense column already carries every other channel's bits, so a
            // rebuild would re-evaluate queries the edit never mentioned.
            let touched = self.interactions.restated_channels(spec);
            if touched.is_empty() {
                let channels = self.interactions.channels(spec);
                let states = crate::scene::interaction::resolve_states(scene, &channels)?;
                crate::scene::interaction::install(scene, states)?;
            } else {
                for (channel, next) in touched {
                    crate::scene::interaction::update_channel(scene, channel, next.as_ref())?;
                }
            }
        }
        let (rules, prepared) = self.appearance.into_parts();
        let installed = match prepared {
            Some(prepared) => Some(crate::scene::appearance::install(
                scene, appearance, prepared,
            )?),
            None => None,
        };
        let mut physical = self.physical;
        let released = retarget(&self.targets, &mut physical, scene, selections)?;
        if let Some(installed) = &installed {
            bind_overlays(
                installed,
                &mut physical,
                &self.representations,
                spec,
                scene,
                handles,
            );
        }
        scene.replace_representations(
            physical
                .into_iter()
                .map(|(_, handle, representation)| (handle, representation))
                .collect(),
        )?;
        for previous in released {
            // A selection another representation still draws stays.
            if scene.remove_selection(previous).is_ok() {
                selections.retain(|_, handle| *handle != previous);
            }
        }
        if let Some(installed) = installed {
            for column in installed.retired {
                let _ = scene.remove_atom_property(column);
            }
        }
        for (id, representation) in self.representations {
            let _ = spec.representations.insert(id, representation);
        }
        for (id, visual) in self.visual_updates {
            if let Some(visual) = visual {
                let _ = visuals.insert(id, visual);
            } else {
                let _ = visuals.remove(&id);
            }
        }
        AppearanceUpdates::commit_spec(rules, spec);
        self.interactions.commit(spec);
        self.science.commit(spec);
        assign(&mut spec.camera, self.camera);
        if self.touched {
            spec.revision = spec.revision.wrapping_add(1);
        }
        Ok(())
    }
}

/// Points every retargeted representation at its new rows, sharing a selection
/// another representation already draws, and returns the selections it left.
fn retarget(
    targets: &super::targets::TargetUpdates,
    physical: &mut [(
        RepresentationId,
        RepresentationHandle,
        molgfx_core::Representation,
    )],
    scene: &mut molgfx_core::Scene,
    selections: &mut BTreeMap<String, SelectionHandle>,
) -> Result<Vec<SelectionHandle>, Error> {
    let mut released = Vec::new();
    for (id, _, representation) in physical.iter_mut() {
        let Some(prepared) = targets.prepared.get(id) else {
            continue;
        };
        let shared = selections
            .get(&prepared.key)
            .copied()
            .filter(|handle| scene.selection(*handle).is_some());
        let selection = if let Some(shared) = shared {
            shared
        } else {
            let created = scene.add_query_selection(
                prepared.structure,
                (*prepared.rows).clone(),
                prepared.fingerprint,
            )?;
            let _ = selections.insert(prepared.key.clone(), created);
            created
        };
        if let RepresentationTarget::Selection(previous) = representation.target
            && previous != selection
        {
            released.push(previous);
        }
        representation.target = RepresentationTarget::Selection(selection);
    }
    Ok(released)
}

/// Binds each touched structure's overlay to every representation of it,
/// including ones the patch did not otherwise change.
fn bind_overlays(
    installed: &crate::scene::appearance::Installed,
    physical: &mut Vec<(
        RepresentationId,
        RepresentationHandle,
        molgfx_core::Representation,
    )>,
    staged: &BTreeMap<RepresentationId, crate::representation::form::RepresentationSpec>,
    spec: &SceneSpec,
    scene: &molgfx_core::Scene,
    handles: &BTreeMap<RepresentationId, RepresentationHandle>,
) {
    let structure_of = |id: &RepresentationId| -> Option<StructureId> {
        staged
            .get(id)
            .or_else(|| spec.representations.get(id))
            .and_then(|representation| representation.common.structure)
    };
    let overlay_of = |id: &RepresentationId| {
        structure_of(id).and_then(|structure| installed.overlays.get(&structure).copied())
    };
    for (id, _, representation) in physical.iter_mut() {
        if let Some(overlay) = overlay_of(id) {
            representation.color_overlay = overlay;
        }
    }
    for (id, handle) in handles {
        if physical.iter().any(|(touched, _, _)| touched == id) {
            continue;
        }
        let Some(overlay) = overlay_of(id) else {
            continue;
        };
        let Some(mut representation) = scene.representation(*handle).cloned() else {
            continue;
        };
        representation.color_overlay = overlay;
        physical.push((*id, *handle, representation));
    }
}
