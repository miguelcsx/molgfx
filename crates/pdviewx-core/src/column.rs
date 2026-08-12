//! Owned, revision-counted columns.
//!
//! A column is dense per-atom (or per-entity) state the scene owns, as
//! opposed to state borrowed from the parsed structure. Every mutation bumps
//! the revision, which is what lets the renderer upload only what changed:
//! a frame with no edits uploads nothing.

#[cfg(test)]
#[path = "column_tests.rs"]
mod tests;

/// Converts a length to `u32`, saturating at the maximum. Table sizes are
/// bounded far below the limit; saturation only keeps the conversion total.
pub(crate) fn saturating_u32(n: usize) -> u32 {
    match u32::try_from(n) {
        Ok(v) => v,
        Err(_) => u32::MAX,
    }
}

/// A monotonically increasing change counter.
///
/// Comparing two revisions answers "has this changed since I last looked"
/// in `O(1)` without diffing contents.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct Revision(u64);

impl Revision {
    /// The revision before any change.
    pub const INITIAL: Self = Self(0);

    /// The next revision.
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// The raw counter value.
    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }
}

/// A dense owned column with a change counter.
///
/// Reads are free; any mutable access bumps the revision, so callers take
/// `values_mut` only when actually writing.
#[derive(Clone, Debug, Default)]
pub struct Column<T> {
    values: Vec<T>,
    revision: Revision,
}

impl<T> Column<T> {
    /// Builds a column from its values.
    #[must_use]
    pub fn new(values: Vec<T>) -> Self {
        Self {
            values,
            revision: Revision::INITIAL,
        }
    }

    /// Number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// True when the column has no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Read-only view of the values.
    #[must_use]
    pub fn values(&self) -> &[T] {
        &self.values
    }

    /// Mutable view of the values; bumps the revision.
    pub fn values_mut(&mut self) -> &mut [T] {
        self.revision = self.revision.next();
        &mut self.values
    }

    /// The current change counter.
    #[must_use]
    pub fn revision(&self) -> Revision {
        self.revision
    }
}

impl<T: bytemuck::Pod> Column<T> {
    /// The values as raw bytes, for direct upload.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.values)
    }
}
