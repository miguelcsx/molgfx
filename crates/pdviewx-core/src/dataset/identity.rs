//! Stable dataset identities and checked chunk-local addressing.

use core::fmt;

/// Stable identity of one logical dataset.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DatasetId(u64);

impl DatasetId {
    /// Dataset identity used by transitional constructors that receive only a
    /// parsed structure and therefore have no caller-owned global identity.
    pub const LEGACY: Self = Self(0);

    /// Creates an identity from the caller's stable value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the caller's stable value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for DatasetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Stable identity of one chunk within a dataset.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ChunkId(u64);

impl ChunkId {
    /// Creates an identity from the caller's stable value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the caller's stable value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ChunkId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Stable caller identity for one spatial occurrence of a resident chunk.
///
/// A chunk can be drawn more than once with distinct transforms or shared
/// templates. Spatial relations therefore address an occurrence as well as a
/// dataset, chunk and logical row.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ChunkOccurrenceId(u64);

impl ChunkOccurrenceId {
    /// Creates a stable caller-owned occurrence identity.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the underlying stable value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ChunkOccurrenceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Stable row identity in the full logical dataset.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct LogicalRow(u64);

impl LogicalRow {
    /// Creates a logical row from the caller's stable value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the full-dataset row value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for LogicalRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Dense row index valid only inside one resident chunk.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct LocalRow(u32);

impl LocalRow {
    /// Creates a chunk-local row.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the local GPU-compatible index.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
#[path = "identity_tests.rs"]
mod tests;
