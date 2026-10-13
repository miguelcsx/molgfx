//! Dense semantic interaction state shared by every representation.

use super::interaction_column::RowSpan;
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
        self.interaction_states
            .get(&structure)
            .map(super::interaction_column::InteractionColumn::column)
    }

    /// Revision shared by all semantic interaction-state columns.
    #[must_use]
    pub const fn interaction_state_revision(&self) -> u64 {
        self.interaction_state_revision
    }

    /// Rows of one structure's state words edited after revision `since`.
    ///
    /// `None` means the history cannot prove which rows changed, so the
    /// consumer must treat every row as dirty.
    #[must_use]
    pub fn interaction_state_dirty(
        &self,
        structure: StructureHandle,
        since: u64,
    ) -> Option<std::ops::Range<u32>> {
        self.interaction_states.get(&structure)?.dirty_since(since)
    }

    /// Applies one channel's new bit set, retiring the one already stored.
    ///
    /// A channel edit is not a rebuild. Rebuilding every column re-evaluates
    /// every declared query and walks the whole molecule per channel, so the
    /// edit an application performs per frame — the hover that moves between
    /// atoms — would cost the size of the structure. Visiting selected rows and clearing the gaps costs the molecule once
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
        if column.column().len() != atom_count as usize {
            return Err(CoreError::InvalidSelection {
                reason: "interaction-state row count does not match its structure",
            });
        }
        let words = column.words_mut();
        let mut changed = false;
        let mut cursor = 0;
        let mut span = RowSpan::default();
        if let Some(rows) = rows {
            rows.for_each(atom_count, |row| {
                let row = row as usize;
                if row < cursor {
                    return;
                }
                for (index, word) in (cursor..row).zip(&mut words[cursor..row]) {
                    if *word & mask != 0 {
                        *word &= !mask;
                        span.add(crate::column::saturating_u32(index));
                    }
                }
                if words[row] & mask != mask {
                    words[row] |= mask;
                    span.add(crate::column::saturating_u32(row));
                }
                cursor = row + 1;
            });
        }
        for (index, word) in (cursor..).zip(&mut words[cursor..]) {
            if *word & mask != 0 {
                *word &= !mask;
                span.add(crate::column::saturating_u32(index));
            }
        }
        let rows = span.rows();
        changed |= !rows.is_empty();
        column.record(rows);
        self.hover_memory.forget();
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
            let words = column.column().values();
            let Some(first) = words.iter().zip(&values).position(|(a, b)| a != b) else {
                continue;
            };
            let last = match words.iter().zip(&values).rposition(|(a, b)| a != b) {
                Some(last) => last,
                None => first,
            };
            column.words_mut().copy_from_slice(&values);
            column.record(
                crate::column::saturating_u32(first)..crate::column::saturating_u32(last + 1),
            );
            changed = true;
        }
        self.hover_memory.forget();
        if changed {
            self.interaction_state_revision = self.interaction_state_revision.wrapping_add(1);
        }
        Ok(())
    }
}
