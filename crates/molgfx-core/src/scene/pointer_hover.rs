//! Allocation-free pointer state updates; O(atoms) for at most two picked rows.
use crate::{CoreError, InteractionState, Scene, StructureHandle};

impl Scene {
    /// Replaces pointer hover across structures, preserving every other channel.
    /// Repeating the same target leaves the interaction revision unchanged.
    ///
    /// # Errors
    /// Returns an error for unknown structures, unowned rows or malformed columns.
    pub fn update_pointer_hover(
        &mut self,
        target: Option<StructureHandle>,
        rows: &[u32],
    ) -> Result<bool, CoreError> {
        if rows.len() > 2 {
            return Err(CoreError::InvalidSelection {
                reason: "pointer hover accepts an atom or two bond endpoints",
            });
        }
        if let Some(handle) = target {
            let placed = self.structure(handle).ok_or(CoreError::StaleHandle)?;
            if rows.iter().any(|row| *row >= placed.atoms.len()) {
                return Err(CoreError::InvalidSelection {
                    reason: "hover row is outside its structure",
                });
            }
        } else if !rows.is_empty() {
            return Err(CoreError::InvalidSelection {
                reason: "hover rows require a structure",
            });
        }
        for (handle, column) in &self.interaction_states {
            let placed = self.structure(*handle).ok_or(CoreError::StaleHandle)?;
            if column.values().len() != placed.atoms.len() as usize {
                return Err(CoreError::InvalidSelection {
                    reason: "interaction-state row count does not match its structure",
                });
            }
        }
        let mask = InteractionState::HOVERED.bits();
        let mut changed = false;
        for (handle, column) in &mut self.interaction_states {
            for (row, word) in (0_u32..).zip(column.values_mut()) {
                let hovered = Some(*handle) == target && rows.contains(&row);
                let next = if hovered { *word | mask } else { *word & !mask };
                changed |= next != *word;
                *word = next;
            }
        }
        if changed {
            self.interaction_state_revision = self.interaction_state_revision.wrapping_add(1);
        }
        Ok(changed)
    }
}
