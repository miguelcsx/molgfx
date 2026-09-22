//! Dense semantic interaction state shared by every representation.

use crate::{AtomSelection, Column, CoreError, Scene, StructureHandle};

/// One bit in the per-atom semantic interaction word.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InteractionState(u32);

impl InteractionState {
    /// User selection.
    pub const SELECTED: Self = Self(1);
    /// Current pointer hover.
    pub const HOVERED: Self = Self(1 << 1);
    /// Active focus target.
    pub const FOCUSED: Self = Self(1 << 2);
    /// De-emphasized context.
    pub const MUTED: Self = Self(1 << 3);
    /// Explicitly hidden entity.
    pub const HIDDEN: Self = Self(1 << 4);
    /// Number of built-in interaction bits.
    pub const BUILTIN_COUNT: u32 = 5;
    /// Total number of channels carried by one state word.
    pub const CAPACITY: u32 = u32::BITS;

    /// A bounded custom channel.
    #[must_use]
    pub const fn custom(index: u32) -> Option<Self> {
        let bit = index.saturating_add(Self::BUILTIN_COUNT);
        if bit < Self::CAPACITY {
            Some(Self(1 << bit))
        } else {
            None
        }
    }

    /// GPU-visible bit mask.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl Scene {
    /// Dense per-atom state words for one structure.
    #[must_use]
    pub fn interaction_state(&self, structure: StructureHandle) -> Option<&Column<u32>> {
        self.interaction_states.get(&structure)
    }

    /// Revision shared by all semantic interaction-state columns.
    #[must_use]
    pub const fn interaction_state_revision(&self) -> u64 {
        self.interaction_state_revision
    }

    /// Applies one channel's new bit set, retiring the one already stored.
    ///
    /// A channel edit is not a rebuild. Rebuilding every column re-evaluates
    /// every declared query and walks the whole molecule per channel, so the
    /// edit an application performs per frame — the hover that moves between
    /// atoms — would cost the size of the structure. Clearing first and then
    /// setting exactly the rows the new query selects costs the molecule once
    /// per edit, not once per channel.
    ///
    /// The column itself is the authority on which rows used to carry the bit,
    /// so retiring it needs no query and no second selection: the previous
    /// query is not re-evaluated, which keeps an edit to a single evaluation
    /// however the channel moved.
    ///
    /// # Errors
    ///
    /// Returns a stale-handle or row-count error without changing the scene.
    pub fn update_interaction_channel(
        &mut self,
        handle: StructureHandle,
        mask: u32,
        rows: Option<&AtomSelection>,
    ) -> Result<(), CoreError> {
        let placed = self.structure(handle).ok_or(CoreError::StaleHandle)?;
        let atom_count = placed.atoms.len();
        let column = self
            .interaction_states
            .get_mut(&handle)
            .ok_or(CoreError::StaleHandle)?;
        let words = column.values_mut();
        if words.len() != atom_count as usize {
            return Err(CoreError::InvalidSelection {
                reason: "interaction-state row count does not match its structure",
            });
        }
        let mut changed = false;
        for word in words.iter_mut() {
            let cleared = *word & !mask;
            changed |= cleared != *word;
            *word = cleared;
        }
        if let Some(rows) = rows {
            rows.for_each(atom_count, |row| {
                if let Some(word) = words.get_mut(row as usize)
                    && *word & mask == 0
                {
                    *word |= mask;
                    changed = true;
                }
            });
        }
        if changed {
            self.interaction_state_revision = self.interaction_state_revision.wrapping_add(1);
        }
        Ok(())
    }

    /// Atomically replaces structure-scoped semantic state columns.
    ///
    /// # Errors
    ///
    /// Returns a stale-handle or row-count error without changing the scene.
    pub fn replace_interaction_states(
        &mut self,
        states: Vec<(StructureHandle, Vec<u32>)>,
    ) -> Result<(), CoreError> {
        for (structure, values) in &states {
            let placed = self.structure(*structure).ok_or(CoreError::StaleHandle)?;
            if values.len() != placed.atoms.len() as usize {
                return Err(CoreError::InvalidSelection {
                    reason: "interaction-state row count does not match its structure",
                });
            }
        }
        let mut changed = false;
        for (structure, values) in states {
            let column = self
                .interaction_states
                .get_mut(&structure)
                .ok_or(CoreError::StaleHandle)?;
            if column.values() != values {
                column.values_mut().copy_from_slice(&values);
                changed = true;
            }
        }
        if changed {
            self.interaction_state_revision = self.interaction_state_revision.wrapping_add(1);
        }
        Ok(())
    }
}
