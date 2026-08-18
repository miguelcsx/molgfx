//! Atom selections: which rows a representation or query applies to.
//!
//! A selection produces indices only; nothing materializes until a
//! representation draws. The encodings trade size for speed across selection
//! shapes — contiguous ranges, scattered picks, dense masks, and compressed
//! bitmaps for large sparse sets. Boolean composition works pairwise in
//! `O(min(|a|, |b|))` for the bitmap forms and `O(n)` otherwise.

use roaring::RoaringBitmap;
use smallvec::SmallVec;
use std::ops::Range;

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;

/// A set of atom rows.
#[derive(Clone, Debug, Default)]
pub enum AtomSelection {
    /// No atoms.
    #[default]
    Empty,
    /// Every atom in the table.
    All,
    /// One contiguous run.
    Range(Range<u32>),
    /// A few contiguous runs, ascending and disjoint.
    Ranges(SmallVec<[Range<u32>; 4]>),
    /// Scattered indices, ascending.
    Sparse(Vec<u32>),
    /// A dense bitmask over the whole table.
    Dense(pdbiox::BitVec),
    /// A compressed bitmap; the right shape for large scattered sets.
    Roaring(RoaringBitmap),
}

impl AtomSelection {
    /// Number of selected rows, given the table length that `All` and
    /// `Dense` are relative to.
    #[must_use]
    pub fn count(&self, table_len: u32) -> u64 {
        match self {
            Self::Empty => 0,
            Self::All => u64::from(table_len),
            Self::Range(r) => u64::from(r.end.saturating_sub(r.start)),
            Self::Ranges(rs) => rs
                .iter()
                .map(|r| u64::from(r.end.saturating_sub(r.start)))
                .sum(),
            Self::Sparse(v) => v.len() as u64,
            Self::Dense(bits) => bits.ones().filter(|&i| i < table_len).count() as u64,
            Self::Roaring(map) => map.len(),
        }
    }

    /// Whether a row is selected.
    #[must_use]
    pub fn contains(&self, index: u32) -> bool {
        match self {
            Self::Empty => false,
            Self::All => true,
            Self::Range(r) => r.contains(&index),
            Self::Ranges(rs) => rs.iter().any(|r| r.contains(&index)),
            Self::Sparse(v) => v.binary_search(&index).is_ok(),
            Self::Dense(bits) => bits.test(index),
            Self::Roaring(map) => map.contains(index),
        }
    }

    /// Visits every selected row in ascending order; `O(selected)` for every
    /// encoding. The single iteration path every consumer shares.
    pub fn for_each(&self, table_len: u32, mut visit: impl FnMut(u32)) {
        match self {
            Self::Empty => {}
            Self::All => {
                for i in 0..table_len {
                    visit(i);
                }
            }
            Self::Range(r) => {
                for i in r.start..r.end.min(table_len) {
                    visit(i);
                }
            }
            Self::Ranges(rs) => {
                for r in rs {
                    for i in r.start..r.end.min(table_len) {
                        visit(i);
                    }
                }
            }
            Self::Sparse(v) => {
                for &i in v {
                    if i < table_len {
                        visit(i);
                    }
                }
            }
            Self::Dense(bits) => {
                for i in bits.ones() {
                    if i < table_len {
                        visit(i);
                    }
                }
            }
            Self::Roaring(map) => {
                for i in map {
                    if i < table_len {
                        visit(i);
                    }
                }
            }
        }
    }

    /// Normalizes to a compressed bitmap, the common currency of
    /// composition. `O(selected)`.
    #[must_use]
    pub fn to_bitmap(&self, table_len: u32) -> RoaringBitmap {
        match self {
            Self::Roaring(map) => map.clone(),
            Self::Range(r) => {
                let mut map = RoaringBitmap::new();
                map.insert_range(r.start..r.end.min(table_len));
                map
            }
            _ => {
                let mut map = RoaringBitmap::new();
                self.for_each(table_len, |i| {
                    map.insert(i);
                });
                map
            }
        }
    }

    /// Set union.
    #[must_use]
    pub fn union(&self, other: &Self, table_len: u32) -> Self {
        match (self, other) {
            (Self::Empty, _) => other.clone(),
            (_, Self::Empty) => self.clone(),
            (Self::All, _) | (_, Self::All) => Self::All,
            _ => Self::Roaring(self.to_bitmap(table_len) | other.to_bitmap(table_len)),
        }
    }

    /// Set intersection.
    #[must_use]
    pub fn intersect(&self, other: &Self, table_len: u32) -> Self {
        match (self, other) {
            (Self::Empty, _) | (_, Self::Empty) => Self::Empty,
            (Self::All, _) => other.clone(),
            (_, Self::All) => self.clone(),
            _ => Self::Roaring(self.to_bitmap(table_len) & other.to_bitmap(table_len)),
        }
    }

    /// Set difference: rows of `self` not in `other`.
    #[must_use]
    pub fn difference(&self, other: &Self, table_len: u32) -> Self {
        match (self, other) {
            (Self::Empty, _) | (_, Self::All) => Self::Empty,
            (_, Self::Empty) => self.clone(),
            _ => Self::Roaring(self.to_bitmap(table_len) - other.to_bitmap(table_len)),
        }
    }
}
