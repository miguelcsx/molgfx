//! What a patch is prepared against.

use crate::id::{RepresentationId, StructureId};
use crate::scene::selection_rows::SelectionRows;
use crate::spec::SceneSpec;
use molgfx_core::{RepresentationHandle, StructureHandle};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub(crate) struct PatchInputs<'a> {
    pub(crate) spec: &'a SceneSpec,
    pub(crate) scene: &'a molgfx_core::Scene,
    pub(crate) handles: &'a BTreeMap<RepresentationId, RepresentationHandle>,
    pub(crate) visuals: &'a BTreeMap<RepresentationId, crate::visual::ResolvedVisual>,
    pub(crate) properties: &'a BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
    pub(crate) structures: &'a BTreeMap<StructureId, molgfx_core::MolecularSource>,
    pub(crate) property_bindings: &'a BTreeMap<Box<str>, crate::ScalarPropertyBinding>,
    pub(crate) overlay_bindings: &'a crate::overlay::OverlayBindings,
    pub(crate) overlay: &'a crate::overlay::lower::LoweredOverlay,
    pub(crate) structure_assets: &'a crate::scene::runtime::StructureAssets,
    pub(crate) rows: &'a SelectionRows,
}

impl PatchInputs<'_> {
    /// The core handle of every bound structure, keyed by semantic identity.
    pub(super) fn structure_handles(&self) -> BTreeMap<StructureId, StructureHandle> {
        self.structures
            .keys()
            .copied()
            .zip(self.scene.structures().map(|(handle, _)| handle))
            .collect()
    }
}
