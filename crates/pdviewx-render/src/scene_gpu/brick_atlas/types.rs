//! Public configuration, lifecycle records and typed failures.

use pdviewx_core::{BrickDescriptor, BrickId};
use pdviewx_gpu::{FenceValue, UploadBackpressure, UploadRingConfig};
use pdviewx_semantic::BrickWorkingSetError;

/// Physical texel interpretation shared by one atlas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrickAtlasKind {
    /// Scalar density or scientific field.
    Scalar,
    /// Exact categorical labels.
    Segmentation,
    /// Occupancy values represented as scalar probabilities.
    Occupancy,
    /// Scalar field consumed by surface extraction and rendering.
    Surface,
}

impl BrickAtlasKind {
    pub(super) const fn code(self) -> u32 {
        match self {
            Self::Scalar => 1,
            Self::Segmentation => 2,
            Self::Occupancy => 3,
            Self::Surface => 4,
        }
    }
}

/// Immutable fixed budgets for one sparse atlas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BrickAtlasConfig {
    /// Common stored shape, including symmetric halos.
    pub stored_shape: [u16; 3],
    /// Maximum simultaneously mapped bricks.
    pub resident_capacity: usize,
    /// Fixed upload-ring budgets.
    pub uploads: UploadRingConfig,
    /// Value interpretation and texture format.
    pub kind: BrickAtlasKind,
}

/// Borrowed provider completion. Exactly four bytes are stored per voxel.
#[derive(Clone, Copy, Debug)]
pub struct BrickAtlasUpload<'a> {
    /// Catalog descriptor carrying global address, mip and generation.
    pub descriptor: BrickDescriptor,
    /// Tightly packed x-fastest stored voxels, including halos.
    pub bytes: &'a [u8],
}

/// Cumulative bounded-working-set telemetry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BrickAtlasMetrics {
    /// Bytes permanently reserved by the physical atlas.
    pub atlas_bytes: u64,
    /// Bytes permanently reserved by the GPU page table.
    pub page_table_bytes: u64,
    /// Currently published pages.
    pub resident_pages: usize,
    /// Uploads waiting on a fence.
    pub pending_uploads: usize,
    /// Evictions waiting on a fence.
    pub pending_evictions: usize,
    /// Older provider completions discarded after retirement.
    pub stale_completions: u64,
    /// Number of page-table uploads; unchanged on stable frames.
    pub page_table_writes: u64,
}

/// Work retired by one non-blocking poll.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BrickAtlasPoll {
    /// Greatest backend fence observed complete.
    pub completed_fence: FenceValue,
    /// Newly published current generations.
    pub uploads_published: usize,
    /// Stale generations discarded.
    pub stale_completions: usize,
    /// Slots made reusable after eviction.
    pub evictions_completed: usize,
}

/// Sparse-atlas validation, capacity and lifecycle failures.
#[derive(Debug, thiserror::Error)]
pub enum BrickAtlasError {
    /// Fixed dimensions or capacities cannot form a portable atlas.
    #[error("invalid sparse brick atlas configuration")]
    InvalidConfiguration,
    /// A descriptor does not use the atlas's common stored shape.
    #[error("brick {brick} stored shape does not match the atlas")]
    ShapeMismatch {
        /// Global brick whose stored extent differs.
        brick: BrickId,
    },
    /// The payload byte count does not match the descriptor shape.
    #[error("brick {brick} payload has {received} bytes; expected {expected}")]
    PayloadSize {
        /// Global brick carrying malformed payload bytes.
        brick: BrickId,
        /// Exact bytes implied by stored voxel count.
        expected: usize,
        /// Provider bytes received.
        received: usize,
    },
    /// Metadata semantics do not match the atlas texel interpretation.
    #[error("brick {brick} value semantics do not match the atlas")]
    KindMismatch {
        /// Global brick whose scientific value kind differs.
        brick: BrickId,
    },
    /// All fixed lifecycle records are occupied.
    #[error("sparse brick lifecycle capacity {capacity} is exhausted")]
    LifecycleCapacity {
        /// Fixed lifecycle or atlas count ceiling.
        capacity: usize,
    },
    /// A duplicate eviction is already pending.
    #[error("brick {brick} already has an eviction pending")]
    EvictionPending {
        /// Global brick already waiting on a retirement fence.
        brick: BrickId,
    },
    /// Existing logical working-set validation failed.
    #[error(transparent)]
    WorkingSet(#[from] BrickWorkingSetError),
    /// Fixed upload staging applied typed backpressure.
    #[error(transparent)]
    Upload(#[from] UploadBackpressure),
    /// Device creation, submission polling or limits failed.
    #[error(transparent)]
    Gpu(#[from] pdviewx_gpu::GpuError),
}
