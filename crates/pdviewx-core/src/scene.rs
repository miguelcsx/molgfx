//! The scene: placed structures, stored selections and representations.
//!
//! The scene is the renderable model. It owns handles, per-table revisions
//! and the representation list; it holds no GPU state and issues no device
//! commands. Structure edits never touch coordinates — those are borrowed —
//! so a coordinate change is a swap of the referenced model, tracked by the
//! parser's own generation counter.

use crate::atoms::AtomTable;
use crate::error::CoreError;
use crate::handle::{RepresentationHandle, SelectionHandle, SlotMap, StructureHandle};
use crate::placed::PlacedStructure;
use crate::representation::{Representation, RepresentationKind};
use crate::selection::AtomSelection;
use pdviewx_math::Aabb;

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;

/// The renderable model of one or more structures.
#[derive(Debug, Default)]
pub struct Scene {
    structures: SlotMap<PlacedStructure>,
    selections: SlotMap<AtomSelection>,
    representations: SlotMap<Representation>,
    /// Bumped whenever the representation list or its parameters change;
    /// keys the renderer's slot table rebuild.
    representation_revision: u64,
}

impl Scene {
    /// An empty scene.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a scene containing one structure and no representations.
    ///
    /// # Errors
    ///
    /// Fails when the structure carries no dense coordinate block to borrow.
    pub fn from_structure(structure: &pdbiox::Structure) -> Result<Self, CoreError> {
        let mut scene = Self::new();
        scene.add_structure(structure)?;
        Ok(scene)
    }

    /// Places a structure into the scene at the identity transform.
    ///
    /// # Errors
    ///
    /// Fails when the structure carries no dense coordinate block to borrow.
    pub fn add_structure(
        &mut self,
        structure: &pdbiox::Structure,
    ) -> Result<StructureHandle, CoreError> {
        let placed = PlacedStructure::new(structure).ok_or(CoreError::StructureRead {
            summary: "structure has no dense coordinate block".to_owned(),
        })?;
        Ok(StructureHandle(self.structures.insert(placed)))
    }

    /// Removes a placed structure; its handle and dependent representations
    /// become stale.
    pub fn remove_structure(&mut self, handle: StructureHandle) -> Option<PlacedStructure> {
        self.representation_revision += 1;
        self.structures.remove(handle.0)
    }

    /// Resolves a structure handle.
    #[must_use]
    pub fn structure(&self, handle: StructureHandle) -> Option<&PlacedStructure> {
        self.structures.get(handle.0)
    }

    /// Mutable resolution of a structure handle.
    pub fn structure_mut(&mut self, handle: StructureHandle) -> Option<&mut PlacedStructure> {
        self.structures.get_mut(handle.0)
    }

    /// Iterates placed structures in stable slot order.
    pub fn structures(&self) -> impl Iterator<Item = (StructureHandle, &PlacedStructure)> + '_ {
        self.structures.iter().map(|(h, s)| (StructureHandle(h), s))
    }

    /// The first placed structure's atom table, if any; the common case of a
    /// single-structure scene.
    #[must_use]
    pub fn first_atoms(&self) -> Option<&AtomTable> {
        self.structures.iter().next().map(|(_, s)| &s.atoms)
    }

    /// Stores a selection and returns its handle.
    pub fn add_selection(&mut self, selection: AtomSelection) -> SelectionHandle {
        SelectionHandle(self.selections.insert(selection))
    }

    /// Resolves a selection handle.
    #[must_use]
    pub fn selection(&self, handle: SelectionHandle) -> Option<&AtomSelection> {
        self.selections.get(handle.0)
    }

    /// Adds a representation of `kind` over a stored selection.
    ///
    /// # Errors
    ///
    /// Fails on a stale selection handle, or a kind the engine cannot draw
    /// yet.
    pub fn represent(
        &mut self,
        selection: SelectionHandle,
        kind: RepresentationKind,
    ) -> Result<RepresentationHandle, CoreError> {
        if self.selections.get(selection.0).is_none() {
            return Err(CoreError::StaleHandle);
        }
        match kind {
            RepresentationKind::Spacefill | RepresentationKind::BallAndStick => {}
            other => return Err(CoreError::Unsupported { kind: other }),
        }
        self.representation_revision += 1;
        let handle = self
            .representations
            .insert(Representation::new(selection, kind));
        Ok(RepresentationHandle(handle))
    }

    /// Resolves a representation handle.
    #[must_use]
    pub fn representation(&self, handle: RepresentationHandle) -> Option<&Representation> {
        self.representations.get(handle.0)
    }

    /// Mutable resolution of a representation; bumps the representation
    /// revision, since any field edit can change what draws.
    pub fn representation_mut(
        &mut self,
        handle: RepresentationHandle,
    ) -> Option<&mut Representation> {
        self.representation_revision += 1;
        self.representations.get_mut(handle.0)
    }

    /// Hides a representation without removing it.
    pub fn hide(&mut self, handle: RepresentationHandle) {
        if let Some(rep) = self.representations.get_mut(handle.0) {
            rep.visible = false;
            self.representation_revision += 1;
        }
    }

    /// Shows a hidden representation.
    pub fn show(&mut self, handle: RepresentationHandle) {
        if let Some(rep) = self.representations.get_mut(handle.0) {
            rep.visible = true;
            self.representation_revision += 1;
        }
    }

    /// Removes a representation.
    pub fn remove_representation(&mut self, handle: RepresentationHandle) {
        if self.representations.remove(handle.0).is_some() {
            self.representation_revision += 1;
        }
    }

    /// Iterates representations in stable slot order.
    pub fn representations(
        &self,
    ) -> impl Iterator<Item = (RepresentationHandle, &Representation)> + '_ {
        self.representations
            .iter()
            .map(|(h, r)| (RepresentationHandle(h), r))
    }

    /// Number of live representations.
    #[must_use]
    pub fn representation_count(&self) -> usize {
        self.representations.len()
    }

    /// Counter keying the renderer's slot-table rebuild: changes exactly
    /// when the representation list or its parameters change.
    #[must_use]
    pub fn representation_revision(&self) -> u64 {
        self.representation_revision
    }

    /// The world-space bound over every placed structure, `O(atoms)`.
    #[must_use]
    pub fn world_aabb(&self) -> Aabb {
        let mut aabb = Aabb::EMPTY;
        for (_, placed) in self.structures.iter() {
            aabb = aabb.union(&placed.world_aabb());
        }
        aabb
    }
}
