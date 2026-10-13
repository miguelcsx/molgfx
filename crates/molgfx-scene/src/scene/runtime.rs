//! Physical resolution behind the semantic scene boundary.

use crate::error::Error;
use crate::id::StructureId;
use crate::scene::Resolution;
use crate::spec::{SceneSpec, StructureSource};
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

    /// Whether the record describes exactly this structure set, ending at `last`.
    pub(crate) fn covers(&self, count: usize, last: Option<StructureId>) -> bool {
        self.key.len() == count && self.key.last().map(|entry| entry.0) == last
    }

    /// The asset recorded for one structure.
    pub(crate) fn asset(&self, id: StructureId) -> Option<&molgfx_core::StructureAsset> {
        let index = self.key.binary_search_by_key(&id, |entry| entry.0).ok()?;
        self.assets.get(index)
    }

    /// The record without structure `id`, so the next resolution over the
    /// remaining set still reuses every other asset.
    pub(crate) fn without(&self, id: StructureId) -> Self {
        let mut next = self.clone();
        if let Ok(index) = next.key.binary_search_by_key(&id, |entry| entry.0) {
            let _ = next.key.remove(index);
            let _ = next.assets.remove(index);
        }
        next
    }

    /// Appends the asset of a structure whose id follows every recorded one.
    pub(crate) fn push(
        &mut self,
        id: StructureId,
        source: &molgfx_core::MolecularSource,
        asset: molgfx_core::StructureAsset,
    ) {
        self.key
            .push((id, source.identity(), source.coordinate_revision()));
        self.assets.push(asset);
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
/// Returns an invalid-specification error for a representation targeting an
/// unbound structure, and propagates every lowering failure.
pub(crate) fn resolve_reusing(
    spec: &SceneSpec,
    structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    property_bindings: &BTreeMap<Box<str>, crate::ScalarPropertyBinding>,
    overlay_bindings: &crate::overlay::OverlayBindings,
    rows: &crate::scene::selection_rows::SelectionRows,
    reusable: Option<&StructureAssets>,
) -> Result<Resolution, Error> {
    let mut scene = base_scene(structures, reusable)?;
    let structure_handles = structures
        .keys()
        .copied()
        .zip(scene.structures().map(|(handle, _)| handle))
        .collect::<BTreeMap<_, _>>();
    place_structures(spec, &structure_handles, &mut scene)?;
    let channels: Vec<Box<str>> = spec.custom_interactions.keys().cloned().collect();
    let properties = crate::scene::properties::resolve_bindings(
        spec,
        structures,
        property_bindings,
        &structure_handles,
        &mut scene,
    )?;
    let mut appearance = BTreeMap::new();
    let installed = install_appearance(
        spec,
        structures,
        &structure_handles,
        &properties,
        rows,
        &mut scene,
        &mut appearance,
    )?;
    let mut handles = BTreeMap::new();
    let mut selections = BTreeMap::new();
    let mut visuals = BTreeMap::new();
    let mut snfg = Vec::new();
    for (id, representation) in &spec.representations {
        let (structure, core_structure) =
            representation_target(representation, &structure_handles)?;
        if let crate::representation::form::RepresentationFormSpec::Snfg { scale } =
            &representation.form
        {
            // Symbols are instance records for the analytic primitive
            // pipelines: no selection, no native representation. The set is
            // built once per structure revision and only copied here.
            representation.validate_values()?;
            let emitted = emit_snfg(
                &mut scene,
                structures,
                rows,
                representation,
                core_structure,
                *scale,
            )?;
            snfg.push((*id, emitted));
            continue;
        }
        let selection_key = selection_key(representation, structure)?;
        let selection = if let Some(selection) = selections.get(&selection_key).copied() {
            selection
        } else {
            let selection = add_selection(
                &mut scene,
                structures,
                rows,
                representation,
                structure,
                core_structure,
            )?;
            let _ = selections.insert(selection_key, selection);
            selection
        };
        if surface_unbound(representation, overlay_bindings, structure) {
            continue;
        }
        let (native, visual) = representation.native(crate::spec::lowering::Lowering {
            properties: &properties,
            surfaces: Some((overlay_bindings, structures)),
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
    let mut overlay = crate::overlay::lower::lower(spec, &mut overlay_lowering)?;
    overlay.snfg = snfg;
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

/// Applies each structure's declared placement to its placed copy.
fn place_structures(
    spec: &SceneSpec,
    handles: &BTreeMap<StructureId, molgfx_core::StructureHandle>,
    scene: &mut molgfx_core::Scene,
) -> Result<(), Error> {
    for (id, source) in &spec.structures {
        let Some(matrix) = &source.placement else {
            continue;
        };
        StructureSource::validate_placement(matrix)?;
        let placed = handles
            .get(id)
            .and_then(|handle| scene.structure_mut(*handle))
            .ok_or_else(|| Error::InvalidSpec("a placed structure is not bound".to_owned()))?;
        placed.model_to_world = molgfx_math::Mat4::from_cols_array(matrix);
    }
    Ok(())
}

/// A core scene holding every bound structure, built from earlier assets
/// when they were made from the same structure set.
fn base_scene(
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
            let mut scene = molgfx_core::Scene::new();
            for structure in structures.values() {
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

/// Emits one SNFG representation's symbols as primitives; hidden ones emit none.
fn emit_snfg(
    scene: &mut molgfx_core::Scene,
    structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    rows: &crate::scene::selection_rows::SelectionRows,
    representation: &crate::representation::RepresentationSpec,
    core_structure: molgfx_core::StructureHandle,
    scale: f32,
) -> Result<Vec<molgfx_core::PrimitiveHandle>, Error> {
    if !representation.common.visible {
        return Ok(Vec::new());
    }
    let structure = representation
        .common
        .structure
        .ok_or_else(|| Error::InvalidSpec("representation has no structure target".to_owned()))?;
    let source = structures
        .get(&structure)
        .ok_or_else(|| Error::InvalidSpec("representation structure is not bound".to_owned()))?;
    let (selected, _) = rows.rows(structure, source, &representation.common.target)?;
    let symbols = crate::scene::snfg_symbols::symbols_of(source)?;
    let primitives = symbols
        .subset(&selected)
        .to_primitives(core_structure, scale);
    if primitives.is_empty() {
        return Ok(Vec::new());
    }
    Ok(scene.add_primitives(&primitives)?)
}

/// Evaluates one representation's query against its own structure only; an
/// unchanged query over an unchanged molecule comes from the cache.
fn add_selection(
    scene: &mut molgfx_core::Scene,
    structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    rows: &crate::scene::selection_rows::SelectionRows,
    representation: &crate::representation::RepresentationSpec,
    structure: StructureId,
    core_structure: molgfx_core::StructureHandle,
) -> Result<molgfx_core::SelectionHandle, Error> {
    let source = structures
        .get(&structure)
        .ok_or_else(|| Error::InvalidSpec("representation structure is not bound".to_owned()))?;
    let (rows, fingerprint) = rows.rows(structure, source, &representation.common.target)?;
    Ok(scene.add_query_selection(core_structure, (*rows).clone(), fingerprint)?)
}

/// Prepares every ruled structure's class columns and installs them.
fn install_appearance(
    spec: &SceneSpec,
    structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    handles: &BTreeMap<StructureId, molgfx_core::StructureHandle>,
    properties: &BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
    rows: &crate::scene::selection_rows::SelectionRows,
    scene: &mut molgfx_core::Scene,
    columns: &mut crate::scene::appearance::AppearanceColumns,
) -> Result<crate::scene::appearance::Installed, Error> {
    let prepared = crate::scene::appearance::prepare(
        &spec.appearance,
        &crate::scene::appearance::ruled_structures(spec),
        structures,
        handles,
        properties,
        rows,
    )?;
    crate::scene::appearance::install(scene, columns, prepared)
}

/// The structure a representation targets, as an identity and a scene handle.
fn representation_target(
    representation: &crate::representation::RepresentationSpec,
    handles: &BTreeMap<StructureId, molgfx_core::StructureHandle>,
) -> Result<(StructureId, molgfx_core::StructureHandle), Error> {
    let structure = representation
        .common
        .structure
        .ok_or_else(|| Error::InvalidSpec("representation has no structure target".to_owned()))?;
    let core_structure = handles
        .get(&structure)
        .copied()
        .ok_or_else(|| Error::InvalidSpec("representation structure is not bound".to_owned()))?;
    Ok((structure, core_structure))
}

/// The key under which equal queries over one structure share a selection.
fn selection_key(
    representation: &crate::representation::RepresentationSpec,
    structure: StructureId,
) -> Result<String, Error> {
    Ok(format!(
        "{}:{}",
        structure.get(),
        representation.common.target.stable_hash()?
    ))
}

/// Whether a molecular-surface representation has no built surface to draw.
fn surface_unbound(
    representation: &crate::representation::RepresentationSpec,
    overlay_bindings: &crate::overlay::OverlayBindings,
    structure: StructureId,
) -> bool {
    matches!(
        &representation.form,
        crate::representation::form::RepresentationFormSpec::MolecularSurface { source, .. }
            if overlay_bindings
                .molecular_surface(&source.content_hash, structure)
                .is_none()
    )
}
