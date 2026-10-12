//! Keeping the selected bit to one structure.
//!
//! A selection is a query, and a query is evaluated over every structure of the scene, so two
//! structures that share residues (a model and its copy, two designs of one scaffold) both light
//! up when one is clicked. A caller that wants the selection to belong to the structure it was
//! made in names that structure here; the selected bit is then kept off every other one, through
//! every later edit of the scene. Without a scope nothing changes.

use super::Scene;
use crate::error::Error;
use crate::id::StructureId;
use crate::property::registry::StateChannel;

impl Scene {
    /// Limits the selected bit to structure `scope` (`None` lifts the limit and lets the
    /// selection mark every structure it matches again).
    ///
    /// # Errors
    /// Returns an error if the selection no longer evaluates.
    pub fn set_selection_scope(&mut self, scope: Option<StructureId>) -> Result<(), Error> {
        if self.selected_scope == scope {
            return Ok(());
        }
        self.selected_scope = scope;
        // Put the bit back wherever the query matches, then take it off outside the scope.
        super::interaction::update_channel(
            &mut self.resolved,
            StateChannel::Selected,
            self.spec.selected.as_ref(),
        )?;
        self.enforce_selection_scope()
    }

    /// The structure the selected bit is limited to, if any.
    #[must_use]
    pub const fn selection_scope(&self) -> Option<StructureId> {
        self.selected_scope
    }

    /// How many atoms of `structure` carry the selected bit now, or `None` for an unknown one.
    #[must_use]
    pub fn selected_atom_count(&self, structure: StructureId) -> Option<usize> {
        let index = self.structures.keys().position(|id| *id == structure)?;
        let (handle, _) = self.resolved.structures().nth(index)?;
        let column = self.resolved.interaction_state(handle)?;
        let bit = molgfx_core::InteractionState::SELECTED.bits();
        Some(
            column
                .values()
                .iter()
                .filter(|word| *word & bit != 0)
                .count(),
        )
    }

    /// Clears the selected bit on every structure outside the scope. Called after each edit.
    pub(super) fn enforce_selection_scope(&mut self) -> Result<(), Error> {
        let Some(scope) = self.selected_scope else {
            return Ok(());
        };
        let mask = StateChannel::Selected.mask()?;
        let handles: Vec<_> = self
            .resolved
            .structures()
            .map(|(handle, _)| handle)
            .collect();
        for (id, handle) in self.structures.keys().copied().zip(handles) {
            if id == scope {
                continue;
            }
            self.resolved
                .update_interaction_channel(handle, mask, None)
                .map_err(|error| {
                    Error::InvalidSpec(format!("selection scope did not apply: {error}"))
                })?;
        }
        Ok(())
    }
}
