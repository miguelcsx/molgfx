//! Evaluating semantic interaction channels into GPU-resident state bits.
//!
//! A channel is a molecular query; the renderer wants one dense word of state
//! bits per atom. Resolving the two is the only place the semantic channel
//! names meet the physical bit layout, and it happens once per interaction
//! edit rather than once per representation: the state columns are per
//! structure, so representations sharing a structure read the same bits.

use crate::error::Error;
use crate::property::registry::StateChannel;
use crate::selection::Selection;
use crate::spec::SceneSpec;
use molgfx_core::{Scene, StructureHandle};

/// Dense per-structure state words, ready to install.
pub(crate) type States = Vec<(StructureHandle, Vec<u32>)>;

/// Evaluates every channel a scene declares and installs the resulting bits.
///
/// # Errors
///
/// Returns an error if a channel query fails to compile or evaluate, leaving
/// the scene's existing state untouched.
pub(crate) fn write_states(scene: &mut Scene, spec: &SceneSpec) -> Result<(), Error> {
    let states = resolve_states(scene, &channels(spec))?;
    install(scene, states)
}

/// Installs previously resolved bits. Separated from resolution so that a patch
/// can do every fallible step before it mutates anything.
///
/// # Errors
///
/// Returns an error if the rows no longer match the scene.
pub(crate) fn install(scene: &mut Scene, states: States) -> Result<(), Error> {
    scene
        .replace_interaction_states(states)
        .map_err(|error| Error::InvalidSpec(format!("interaction state did not apply: {error}")))
}

/// Resolves channel queries into one dense state word per atom.
///
/// # Errors
///
/// Returns an error if a channel query fails to compile or evaluate.
pub(crate) fn resolve_states(
    scene: &mut Scene,
    channels: &[(StateChannel, Selection)],
) -> Result<States, Error> {
    let structures: Vec<(StructureHandle, usize)> = scene
        .structures()
        .map(|(handle, placed)| (handle, placed.atoms.len() as usize))
        .collect();
    if structures.is_empty() {
        return Ok(Vec::new());
    }
    let mut states: Vec<(StructureHandle, Vec<u32>)> = structures
        .iter()
        .map(|(handle, atoms)| (*handle, vec![0_u32; *atoms]))
        .collect();

    for (channel, selection) in channels {
        let mask = channel.mask()?;
        apply(scene, &mut states, selection, mask)?;
    }
    Ok(states)
}

/// Radius, in ångström, of the residue shell treated as focus context.
///
/// Wide enough to carry the residues lining a binding site, narrow enough that
/// the rest of the structure still recedes. Whole residues are kept rather than
/// individual atoms so a side chain is never cut in half at the boundary.
const CONTEXT_RADIUS: f32 = 6.0;

/// The population a focus de-emphasizes: everything outside the residue shell
/// around the focused subject.
///
/// Written in the same textual form `MolFrame`'s own query builder produces, so
/// it parses exactly as a hand-built expression would.
fn focus_context(focus: &Selection) -> Selection {
    Selection::from(format!(
        "not (byres (within {CONTEXT_RADIUS} of ({})))",
        focus.source()
    ))
}

/// Every declared channel paired with its query, built-ins first so that a
/// custom channel can never take a built-in's bit.
pub(crate) fn channels(spec: &SceneSpec) -> Vec<(StateChannel, Selection)> {
    let builtin = [
        (StateChannel::Selected, spec.selected.as_ref()),
        (StateChannel::Hovered, spec.hovered.as_ref()),
        (StateChannel::Focused, spec.focus.as_ref()),
        (StateChannel::Muted, spec.muted.as_ref()),
        (StateChannel::Hidden, spec.hidden.as_ref()),
    ];
    let mut resolved: Vec<(StateChannel, Selection)> = builtin
        .into_iter()
        .filter_map(|(channel, selection)| selection.map(|selection| (channel, selection.clone())))
        .collect();
    // A focus implies its own context. An explicit muted channel is the
    // caller's own statement about emphasis, so it is never overridden.
    if let (Some(focus), None) = (spec.focus.as_ref(), spec.muted.as_ref()) {
        resolved.push((StateChannel::Muted, focus_context(focus)));
    }
    for (index, selection) in spec.custom_interactions.values().enumerate() {
        let Ok(index) = u32::try_from(index) else {
            break;
        };
        resolved.push((StateChannel::Custom(index), selection.clone()));
    }
    resolved
}

fn apply(
    scene: &mut Scene,
    states: &mut [(StructureHandle, Vec<u32>)],
    selection: &Selection,
    mask: u32,
) -> Result<(), Error> {
    let handle = scene.select_str(selection.source())?;
    for (structure, words) in states.iter_mut() {
        let Some(rows) = scene.selection_for(handle, *structure) else {
            continue;
        };
        let Ok(length) = u32::try_from(words.len()) else {
            continue;
        };
        rows.for_each(length, |row| {
            if let Some(word) = words.get_mut(row as usize) {
                *word |= mask;
            }
        });
    }
    Ok(())
}

/// World-space bounds of the focused subject, if anything carries the bit.
///
/// The viewer frames a focus from this rather than from the whole scene, and it
/// is read from the same state words the shaders read, so what the camera
/// frames and what the shaders emphasize can never disagree.
#[must_use]
pub(crate) fn focus_bounds(scene: &Scene) -> Option<molgfx_math::Aabb> {
    let focused = molgfx_core::InteractionState::FOCUSED.bits();
    let mut bounds = molgfx_math::Aabb::EMPTY;
    let mut any = false;
    for (handle, placed) in scene.structures() {
        let Some(state) = scene.interaction_state(handle) else {
            continue;
        };
        let positions = placed.atoms.coords().slice();
        for (row, word) in state.values().iter().enumerate() {
            if word & focused == 0 {
                continue;
            }
            let Some(position) = positions.get(row) else {
                continue;
            };
            bounds.extend(
                placed
                    .model_to_world
                    .transform_point3(molgfx_math::Vec3::from_array(*position)),
            );
            any = true;
        }
    }
    any.then_some(bounds)
}
