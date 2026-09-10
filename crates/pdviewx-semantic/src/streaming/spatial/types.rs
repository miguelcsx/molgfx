//! Public identities, reports and failures for resident spatial pages.

use pdviewx_core::{ChunkId, ChunkSpan, DatasetId, LocalRow, LogicalRow};
use pdviewx_math::BvhBuildError;

/// Global identity and row interval represented by one resident chunk.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SpatialChunk {
    /// Logical dataset identity.
    pub dataset: DatasetId,
    /// Logical chunk identity within the dataset.
    pub chunk: ChunkId,
    /// Global rows corresponding one-to-one with BLAS primitives.
    pub rows: ChunkSpan,
}

/// Generational address of one bounded resident page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SpatialPageToken {
    page: u32,
    generation: u64,
}

impl SpatialPageToken {
    pub(super) const fn new(page: u32, generation: u64) -> Self {
        Self { page, generation }
    }

    /// Chunk-local resident page index suitable for GPU records.
    #[must_use]
    pub const fn page(self) -> u32 {
        self.page
    }

    /// Generation that rejects stale updates after slot reuse.
    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

/// Conservative primitive candidate returned by TLAS then BLAS traversal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SpatialCandidate {
    /// Global dataset identity.
    pub dataset: DatasetId,
    /// Global chunk identity.
    pub chunk: ChunkId,
    /// Stable row in the logical dataset.
    pub row: LogicalRow,
    /// Compact primitive index valid only in this resident chunk.
    pub local_row: LocalRow,
    /// Resident GPU-compatible page index.
    pub page: u32,
}

/// Work performed by one explicit hierarchy maintenance operation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SpatialMaintenance {
    /// Number of resident chunk bounds visited, independent of logical size.
    pub resident_bounds_visited: u32,
    /// Whether changed membership required deterministic TLAS rebuild.
    pub rebuilt: bool,
}

/// Invalid resident-page mutation or hierarchy state.
#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
pub enum SpatialError {
    /// Resident page capacity cannot fit compact GPU indices.
    #[error("resident spatial capacity {capacity} exceeds u32")]
    CapacityTooLarge {
        /// Requested number of resident pages.
        capacity: usize,
    },
    /// A page index is outside the bounded working set.
    #[error("resident spatial page {page} is outside capacity {capacity}")]
    PageOutsideCapacity {
        /// Requested resident page.
        page: u32,
        /// Configured page capacity.
        capacity: u32,
    },
    /// The page was evicted or reused after the supplied token was issued.
    #[error("resident spatial page {page} generation {generation} is stale")]
    StalePage {
        /// Reused or evicted page.
        page: u32,
        /// Generation supplied by the caller.
        generation: u64,
    },
    /// A reused slot exhausted its generation namespace.
    #[error("resident spatial page {page} exhausted its generation")]
    GenerationExhausted {
        /// Page whose generation cannot advance.
        page: u32,
    },
    /// Primitive count must match the chunk's local row domain.
    #[error("chunk {chunk} declares {expected} rows but supplied {actual} primitive bounds")]
    PrimitiveCountMismatch {
        /// Chunk whose BLAS input is inconsistent.
        chunk: ChunkId,
        /// Declared local primitive count.
        expected: u32,
        /// Supplied primitive count.
        actual: usize,
    },
    /// Every BLAS primitive requires finite, ordered bounds.
    #[error("chunk {chunk} local primitive {local} has invalid bounds")]
    InvalidPrimitiveBounds {
        /// Chunk containing the invalid primitive.
        chunk: ChunkId,
        /// Chunk-local primitive row.
        local: u32,
    },
    /// Queries require all staged bounds to be committed to the TLAS.
    #[error("resident spatial hierarchy has uncommitted changes")]
    UncommittedChanges,
    /// Compact hierarchy offsets exceeded their checked representation.
    #[error(transparent)]
    Hierarchy(#[from] BvhBuildError),
}
