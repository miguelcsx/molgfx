//! The mutable scene state and the resolution that rebuilds it.

use crate::id::{RepresentationId, StructureId};
use crate::spec::SceneSpec;
use molgfx_core::{RepresentationHandle, SelectionHandle};
use std::collections::BTreeMap;

/// Mutable scene state backed by one resolved renderer scene.
#[derive(Debug)]
pub struct Scene {
    pub(super) spec: SceneSpec,
    pub(super) resolved: molgfx_core::Scene,
    pub(super) structures: BTreeMap<StructureId, molgfx_core::MolecularSource>,
    pub(super) representations: BTreeMap<RepresentationId, RepresentationHandle>,
    pub(super) selections: BTreeMap<String, SelectionHandle>,
    pub(super) visuals: BTreeMap<RepresentationId, crate::visual::ResolvedVisual>,
    pub(super) property_bindings: BTreeMap<Box<str>, crate::ScalarPropertyBinding>,
    pub(super) properties: BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
    pub(super) overlay_bindings: crate::overlay::OverlayBindings,
    pub(super) overlay: crate::overlay::lower::LoweredOverlay,
    /// Structure assets of the current resolution, so a later resolution over
    /// the same sources reuses their atom tables instead of rebuilding them.
    pub(super) structure_assets: crate::scene::runtime::StructureAssets,
    /// Evaluated rows by query and molecule identity, shared by every edit.
    pub(super) rows: crate::scene::selection_rows::SelectionRows,
    /// Each structure's appearance class column, for structures with rules.
    pub(super) appearance: crate::scene::appearance::AppearanceColumns,
    pub(super) next_structure: u64,
    pub(super) next_representation: u64,
    /// Immutable bindings removed by a restore, retained for inverse and redo.
    pub(super) snapshot_bindings: super::snapshot::SnapshotBindings,
}

pub(crate) struct Resolution {
    pub(crate) scene: molgfx_core::Scene,
    pub(crate) representations: BTreeMap<RepresentationId, RepresentationHandle>,
    pub(crate) selections: BTreeMap<String, SelectionHandle>,
    pub(crate) visuals: BTreeMap<RepresentationId, crate::visual::ResolvedVisual>,
    pub(crate) properties: BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
    pub(crate) overlay: crate::overlay::lower::LoweredOverlay,
    pub(crate) appearance: crate::scene::appearance::AppearanceColumns,
}
