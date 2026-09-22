//! Dense semantic interaction state shared by every representation.

use crate::{Column, CoreError, Scene, StructureHandle};

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
