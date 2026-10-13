//! Allocation-free pointer state updates; O(picked rows) once the previous
//! target is remembered.
use crate::{CoreError, InteractionState, Scene, StructureHandle};

use super::interaction_column::RowSpan;

/// The rows the last pointer update marked hovered.
///
/// Unknown (the default) whenever another path may have written the hover
/// bit, in which case the next update scans every column once.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct HoverMemory {
    known: bool,
    target: Option<StructureHandle>,
    rows: [u32; 2],
    len: usize,
}

impl HoverMemory {
    /// Drops the remembered target after an outside write to state words.
    pub(crate) const fn forget(&mut self) {
        self.known = false;
    }

    fn same(&self, target: Option<StructureHandle>, rows: &[u32]) -> bool {
        self.known
            && self.target == target
            && rows.iter().all(|row| self.previous().contains(row))
            && self.previous().iter().all(|row| rows.contains(row))
    }

    fn previous(&self) -> &[u32] {
        &self.rows[..self.len]
    }
}

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
        if self.hover_memory.same(target, rows) {
            return Ok(false);
        }
        let changed = if self.hover_memory.known {
            self.move_hover(target, rows)?
        } else {
            self.rescan_hover(target, rows)?
        };
        let mut memory = HoverMemory {
            known: true,
            target,
            rows: [0; 2],
            len: rows.len(),
        };
        memory.rows[..rows.len()].copy_from_slice(rows);
        self.hover_memory = memory;
        if changed {
            self.interaction_state_revision = self.interaction_state_revision.wrapping_add(1);
        }
        Ok(changed)
    }

    /// Clears the remembered rows and sets the new ones: O(picked rows).
    fn move_hover(
        &mut self,
        target: Option<StructureHandle>,
        rows: &[u32],
    ) -> Result<bool, CoreError> {
        let mask = InteractionState::HOVERED.bits();
        let memory = self.hover_memory;
        let mut changed = false;
        for (handle, clear, set) in [
            (memory.target, memory.previous(), &[][..]),
            (target, &[][..], rows),
        ] {
            let Some(handle) = handle else { continue };
            let Some(column) = self.interaction_states.get_mut(&handle) else {
                continue;
            };
            let current = column.column().values();
            let mut span = RowSpan::default();
            for row in clear.iter().chain(set).copied() {
                let Some(word) = current.get(row as usize) else {
                    return Err(CoreError::InvalidSelection {
                        reason: "interaction-state row count does not match its structure",
                    });
                };
                if (*word & mask != 0) != set.contains(&row) {
                    span.add(row);
                }
            }
            if span.is_empty() {
                continue;
            }
            let words = column.words_mut();
            for row in clear {
                words[*row as usize] &= !mask;
            }
            for row in set {
                words[*row as usize] |= mask;
            }
            column.record(span.rows());
            changed = true;
        }
        Ok(changed)
    }

    /// Establishes hover from an unknown state by scanning every column.
    fn rescan_hover(
        &mut self,
        target: Option<StructureHandle>,
        rows: &[u32],
    ) -> Result<bool, CoreError> {
        for (handle, column) in &self.interaction_states {
            let placed = self.structure(*handle).ok_or(CoreError::StaleHandle)?;
            if column.column().len() != placed.atoms.len() as usize {
                return Err(CoreError::InvalidSelection {
                    reason: "interaction-state row count does not match its structure",
                });
            }
        }
        let mask = InteractionState::HOVERED.bits();
        let mut changed = false;
        for (handle, column) in &mut self.interaction_states {
            let owned = Some(*handle) == target;
            let mut span = RowSpan::default();
            for (row, word) in (0_u32..).zip(column.column().values()) {
                if (*word & mask != 0) != (owned && rows.contains(&row)) {
                    span.add(row);
                }
            }
            if span.is_empty() {
                continue;
            }
            for (row, word) in (0_u32..).zip(column.words_mut()) {
                *word = if owned && rows.contains(&row) {
                    *word | mask
                } else {
                    *word & !mask
                };
            }
            column.record(span.rows());
            changed = true;
        }
        Ok(changed)
    }
}
