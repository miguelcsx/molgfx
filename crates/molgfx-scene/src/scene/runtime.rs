//! Physical resolution behind the semantic scene boundary.

use crate::error::Error;
use crate::id::StructureId;
use crate::scene::Resolution;
use crate::spec::SceneSpec;
use std::collections::BTreeMap;

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
    overlay_bindings: &crate::overlay::OverlayBindings,
    rows: &crate::scene::selection_rows::SelectionRows,
) -> Result<Resolution, Error> {
    resolve_reusing(
        spec,
        structures,
        property_bindings,
        overlay_bindings,
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
    overlay_bindings: &crate::overlay::OverlayBindings,
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
        &properties,
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
    let mut overlay_lowering = crate::overlay::lower::OverlayLowering {
        scene: &mut scene,
        structures,
        handles: &structure_handles,
        selections: &mut selections,
        bindings: overlay_bindings,
    };
    let overlay = crate::overlay::lower::lower(spec, &mut overlay_lowering)?;
    crate::scene::interaction::write_states(&mut scene, spec)?;
    Ok(Resolution {
        scene,
        representations: handles,
        selections,
        visuals,
        properties,
        overlay,
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
