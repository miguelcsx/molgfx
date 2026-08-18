//! Interaction-table editing kept separate from the central scene lifecycle.

use crate::{CoreError, EntityKind, EntityRef, InteractionEdge, InteractionHandle, Scene};

#[cfg(test)]
#[path = "interactions_tests.rs"]
mod tests;

impl Scene {
    /// Stores one validated, caller-computed interaction edge.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when the owning structure is absent.
    pub fn add_interaction(
        &mut self,
        interaction: InteractionEdge,
    ) -> Result<InteractionHandle, CoreError> {
        if self.structure(interaction.owner()).is_none() {
            return Err(CoreError::StaleHandle);
        }
        self.interaction_revision = self.interaction_revision.wrapping_add(1);
        Ok(InteractionHandle(self.interactions.insert(interaction)))
    }

    /// Resolves an interaction handle.
    #[must_use]
    pub fn interaction(&self, handle: InteractionHandle) -> Option<&InteractionEdge> {
        self.interactions.get(handle.0)
    }

    /// Resolves an edge returned by GPU picking to its handle and provenance.
    ///
    /// This lookup is `O(1)` and refuses a row whose owning structure does not
    /// match the picked structure attachment.
    #[must_use]
    pub fn interaction_for_entity(
        &self,
        entity: EntityRef,
    ) -> Option<(InteractionHandle, &InteractionEdge)> {
        if entity.kind != EntityKind::Edge {
            return None;
        }
        let (handle, edge) = self.interactions.get_index(entity.index)?;
        if edge.owner() != entity.structure {
            return None;
        }
        Some((InteractionHandle(handle), edge))
    }

    /// Mutable interaction access; any edit invalidates the packed GPU table.
    pub fn interaction_mut(&mut self, handle: InteractionHandle) -> Option<&mut InteractionEdge> {
        let edge = self.interactions.get_mut(handle.0)?;
        self.interaction_revision = self.interaction_revision.wrapping_add(1);
        Some(edge)
    }

    /// Removes an interaction and invalidates its handle.
    pub fn remove_interaction(&mut self, handle: InteractionHandle) -> Option<InteractionEdge> {
        let removed = self.interactions.remove(handle.0);
        if removed.is_some() {
            self.interaction_revision = self.interaction_revision.wrapping_add(1);
        }
        removed
    }

    /// Iterates live interactions in stable scene-table order.
    pub fn interactions(&self) -> impl Iterator<Item = (InteractionHandle, &InteractionEdge)> + '_ {
        self.interactions
            .iter()
            .map(|(handle, edge)| (InteractionHandle(handle), edge))
    }

    /// Number of live caller-supplied interactions.
    #[must_use]
    pub fn interaction_count(&self) -> usize {
        self.interactions.len()
    }

    /// Revision key for the persistent interaction GPU table.
    #[must_use]
    pub const fn interaction_revision(&self) -> u64 {
        self.interaction_revision
    }

    /// Packed edge row for picking and GPU synchronization.
    #[must_use]
    pub fn interaction_row(handle: InteractionHandle) -> u32 {
        handle.row()
    }
}
