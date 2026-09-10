//! Public values at the host-to-GPU paged chunk boundary.

use super::ChunkPlacementError;
use pdviewx_core::{ChunkDomainRef, PayloadKind, ResidencyTicket};
use thiserror::Error;

/// A globally identified structure chunk with chunk-local GPU ranges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidentStructureChunk {
    /// Generation owning this physical allocation.
    pub ticket: ResidencyTicket,
    /// Byte offset in the paged structure arena.
    pub byte_offset: u64,
    /// Uploaded coordinate bytes, excluding page padding.
    pub byte_len: u64,
    /// Number of rows addressable by local `u32` indices.
    pub local_rows: u32,
    /// First cluster descriptor in the bounded cluster arena.
    pub cluster_offset: u32,
    /// Number of fixed-size row clusters.
    pub cluster_count: u32,
}

/// A provider trajectory frame resident in the bounded frame arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidentTrajectoryChunk {
    /// Generation owning this physical allocation.
    pub ticket: ResidencyTicket,
    /// Byte offset in the trajectory-frame arena.
    pub byte_offset: u64,
    /// Uploaded coordinate bytes, excluding page padding.
    pub byte_len: u64,
    /// Number of topology-aligned rows.
    pub local_rows: u32,
}

/// One generic columnar chunk resident in the shared paged source arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidentGenericChunk {
    /// Generation owning this physical allocation.
    pub ticket: ResidencyTicket,
    /// Generic payload layout stored at this range.
    pub kind: PayloadKind,
    /// Exact target table for attribute columns; absent for drawable payloads.
    pub target: Option<ChunkDomainRef>,
    /// Byte offset in the shared paged source arena.
    pub byte_offset: u64,
    /// Uploaded payload bytes, excluding page padding.
    pub byte_len: u64,
    /// Number of chunk-local logical rows.
    pub local_rows: u32,
    /// Physical byte stride of one GPU row. Mixed relation chunks report zero
    /// because their homogeneous partitions have independent strides.
    pub stride: u32,
}

/// Measured storage state for the provider-chunk upload path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkResidencyMetrics {
    /// Fixed paged-arena counters.
    pub arena: pdviewx_gpu::ArenaMetrics,
    /// Fixed upload-ring counters.
    pub uploads: pdviewx_gpu::UploadMetrics,
    /// Number of tracked uploading or resident chunks.
    pub tracked_chunks: usize,
    /// Preallocated tracking capacity.
    pub tracked_capacity: usize,
}

/// Typed failure at the host-to-GPU residency boundary.
#[derive(Debug, Error)]
pub enum ChunkResidencyError {
    /// Host lifecycle or identity validation failed.
    #[error(transparent)]
    Host(#[from] pdviewx_core::HostWorkingSetError),
    /// The payload has no renderer residency implementation.
    #[error("the chunk payload has no GPU residency implementation")]
    UnsupportedPayload,
    /// The host lifecycle claimed readiness without retaining a payload.
    #[error("the active residency ticket has no retained host payload")]
    PayloadMissing,
    /// Fixed tracked-chunk capacity was exhausted.
    #[error("resident chunk tracking capacity is exhausted")]
    TrackingCapacity,
    /// Coordinate bytes cannot be represented by the staging ring.
    #[error("structure chunk byte size exceeds the host address space")]
    SizeOverflow,
    /// A provider returned an empty structure chunk.
    #[error("empty structure chunks cannot become GPU resident")]
    EmptyChunk,
    /// Provider atom metadata did not resolve one locally addressable row.
    #[error("provider atom metadata is missing local row {local_row}")]
    AtomMetadataMissing {
        /// Chunk-local row that could not be decoded.
        local_row: u32,
    },
    /// A compact GPU offset or count exceeded local `u32` addressing.
    #[error("resident chunk GPU range exceeds local u32 addressing")]
    LocalAddressOverflow,
    /// Two trajectory frames or their structure have different row counts.
    #[error("trajectory frame window is not aligned to its structure chunk")]
    TrajectoryTopologyMismatch,
    /// Paged rigid-transform frames do not match their declared occurrence.
    #[error("instance timeline window is duplicated, missing, or row-incompatible")]
    InstanceTimelineMismatch,
    /// Paged attribute frames do not match their stable column contract.
    #[error("attribute timeline window is duplicated, missing, or type-incompatible")]
    AttributeTimelineMismatch,
    /// One global bond endpoint is outside every resident atom page.
    #[error("bond endpoint {dataset:?}:{row:?} is not atom-resident")]
    EndpointNotResident {
        /// Atom dataset owning the endpoint.
        dataset: pdviewx_core::DatasetId,
        /// Global logical atom row.
        row: pdviewx_core::LogicalRow,
    },
    /// A provider bond row could not be projected from retained storage.
    #[error("provider bond row {local_row} is invalid")]
    ProviderBondRecord {
        /// Chunk-local bond row rejected by the provider.
        local_row: u32,
    },
    /// Declarative placement validation failed.
    #[error(transparent)]
    Placement(#[from] ChunkPlacementError),
    /// Provider logical-row metadata could not form a valid chunk span.
    #[error(transparent)]
    Dataset(#[from] pdviewx_core::DatasetError),
    /// Paged GPU allocation failed.
    #[error(transparent)]
    Arena(#[from] pdviewx_gpu::ArenaError),
    /// Upload staging applied explicit backpressure.
    #[error(transparent)]
    Upload(#[from] pdviewx_gpu::UploadBackpressure),
    /// Backend submission/completion failed.
    #[error(transparent)]
    Gpu(#[from] pdviewx_gpu::GpuError),
}
