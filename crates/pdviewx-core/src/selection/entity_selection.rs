//! Adaptive selections over any homogeneous row domain.

#[cfg(test)]
#[path = "entity_selection_tests.rs"]
mod tests;

use crate::{CoreError, RowDomain};
use std::ops::Range;
use std::sync::Arc;

const GPU_BITSET_DENSITY_NUMERATOR: u64 = 1;
const GPU_BITSET_DENSITY_DENOMINATOR: u64 = 8;

/// Canonical CPU selection encoding chosen once from caller rows.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EntitySelectionRows {
    /// No selected rows.
    Empty,
    /// Every row in the domain.
    All,
    /// Sorted disjoint half-open ranges.
    Ranges(Arc<[Range<u32>]>),
    /// Sorted unique rows.
    Sparse(Arc<[u32]>),
}

/// Selection tied to one exact row domain.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EntitySelection {
    domain: RowDomain,
    row_count: u32,
    selected_count: u32,
    rows: EntitySelectionRows,
}

/// GPU selection representation selected from density.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GpuEntitySelection {
    /// Sorted compact row list.
    Compact(Arc<[u32]>),
    /// One bit per source row, least-significant bit first.
    Bitset(Arc<[u32]>),
}

impl EntitySelection {
    /// Builds a canonical selection in `O(n log n)` once, outside frame work.
    ///
    /// # Errors
    ///
    /// Returns a typed error when any row lies outside the domain.
    pub fn from_rows(
        domain: RowDomain,
        row_count: u32,
        mut rows: Vec<u32>,
    ) -> Result<Self, CoreError> {
        if rows.iter().any(|row| *row >= row_count) {
            return Err(invalid("entity selection row is outside its domain"));
        }
        rows.sort_unstable();
        rows.dedup();
        let selected_count = u32::try_from(rows.len())
            .map_err(|_| invalid("entity selection exceeds the portable row limit"))?;
        let encoding = choose_cpu_encoding(row_count, rows);
        Ok(Self {
            domain,
            row_count,
            selected_count,
            rows: encoding,
        })
    }

    /// Selects every row without allocating a row list.
    #[must_use]
    pub const fn all(domain: RowDomain, row_count: u32) -> Self {
        Self {
            domain,
            row_count,
            selected_count: row_count,
            rows: EntitySelectionRows::All,
        }
    }

    /// Exact target domain.
    #[must_use]
    pub const fn domain(&self) -> RowDomain {
        self.domain
    }

    /// Number of selected rows.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.selected_count
    }

    /// Whether no rows are selected.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.selected_count == 0
    }

    /// Canonical cold CPU encoding.
    #[must_use]
    pub const fn rows(&self) -> &EntitySelectionRows {
        &self.rows
    }

    /// Materializes the GPU encoding once when a selection becomes resident.
    #[must_use]
    pub fn gpu_encoding(&self) -> GpuEntitySelection {
        if prefer_bitset(self.selected_count, self.row_count) {
            return GpuEntitySelection::Bitset(self.bitset());
        }
        GpuEntitySelection::Compact(self.compact())
    }

    fn compact(&self) -> Arc<[u32]> {
        match &self.rows {
            EntitySelectionRows::Empty => Arc::from([]),
            EntitySelectionRows::All => (0..self.row_count).collect::<Vec<_>>().into(),
            EntitySelectionRows::Sparse(rows) => Arc::clone(rows),
            EntitySelectionRows::Ranges(ranges) => ranges
                .iter()
                .flat_map(std::clone::Clone::clone)
                .collect::<Vec<_>>()
                .into(),
        }
    }

    fn bitset(&self) -> Arc<[u32]> {
        let words = self.row_count.div_ceil(32) as usize;
        let mut bits = vec![0u32; words];
        match &self.rows {
            EntitySelectionRows::Empty => {}
            EntitySelectionRows::All => bits.fill(u32::MAX),
            EntitySelectionRows::Sparse(rows) => set_rows(&mut bits, rows.iter().copied()),
            EntitySelectionRows::Ranges(ranges) => {
                set_rows(&mut bits, ranges.iter().flat_map(std::clone::Clone::clone));
            }
        }
        if let Some(last) = bits.last_mut() {
            let remainder = self.row_count % 32;
            if remainder != 0 {
                *last &= (1u32 << remainder) - 1;
            }
        }
        bits.into()
    }
}

fn choose_cpu_encoding(row_count: u32, rows: Vec<u32>) -> EntitySelectionRows {
    if rows.is_empty() {
        return EntitySelectionRows::Empty;
    }
    if rows.len() == row_count as usize {
        return EntitySelectionRows::All;
    }
    let mut ranges = Vec::new();
    let mut start = rows[0];
    let mut previous = start;
    for &row in &rows[1..] {
        if row == previous.saturating_add(1) {
            previous = row;
            continue;
        }
        ranges.push(start..previous.saturating_add(1));
        start = row;
        previous = row;
    }
    ranges.push(start..previous.saturating_add(1));
    if ranges.len().saturating_mul(2) < rows.len() {
        EntitySelectionRows::Ranges(ranges.into())
    } else {
        EntitySelectionRows::Sparse(rows.into())
    }
}

const fn prefer_bitset(selected: u32, total: u32) -> bool {
    selected as u64 * GPU_BITSET_DENSITY_DENOMINATOR >= total as u64 * GPU_BITSET_DENSITY_NUMERATOR
}

fn set_rows(rows: &mut [u32], selected: impl Iterator<Item = u32>) {
    for row in selected {
        if let Some(word) = rows.get_mut((row / 32) as usize) {
            *word |= 1 << (row % 32);
        }
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidSelection { reason }
}
