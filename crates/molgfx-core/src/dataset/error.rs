//! Typed failures for paged dataset construction and addressing.

use crate::{ChunkId, LogicalRow, PayloadKind};
use thiserror::Error;

/// A dataset contract was malformed or addressed outside a chunk.
#[derive(Clone, PartialEq, Eq, Debug, Error)]
#[non_exhaustive]
pub enum DatasetError {
    /// A row span contains no rows.
    #[error("chunk row span must not be empty")]
    EmptyRowSpan,
    /// Adding the local count to the first logical row overflowed.
    #[error("chunk row span overflows the u64 logical address space")]
    RowSpanOverflow,
    /// A logical row does not belong to the addressed chunk.
    #[error("logical row {row} is outside chunk {chunk}")]
    LogicalRowOutsideChunk {
        /// Addressed chunk.
        chunk: ChunkId,
        /// Rejected logical row.
        row: LogicalRow,
    },
    /// Two descriptors use the same chunk identity.
    #[error("duplicate chunk identity {chunk}")]
    DuplicateChunk {
        /// Repeated identity.
        chunk: ChunkId,
    },
    /// A descriptor references a parent absent from the catalog.
    #[error("chunk {chunk} references absent parent {parent}")]
    MissingParent {
        /// Child identity.
        chunk: ChunkId,
        /// Missing parent identity.
        parent: ChunkId,
    },
    /// A root or child has an inconsistent hierarchy level.
    #[error("chunk {chunk} has an invalid hierarchy level")]
    InvalidHierarchyLevel {
        /// Invalid descriptor identity.
        chunk: ChunkId,
    },
    /// Spatial bounds contain non-finite or inverted components.
    #[error("chunk bounds must be finite and ordered")]
    InvalidBounds,
    /// Adding footprint components overflowed.
    #[error("chunk footprint overflows u64 bytes")]
    FootprintOverflow,
    /// A catalog index cannot fit the compact in-memory representation.
    #[error("catalog contains more than u32::MAX chunks")]
    CatalogTooLarge,
    /// The catalog has no root from which traversal can begin.
    #[error("dataset catalog needs at least one root chunk")]
    MissingRoot,
    /// A compact provider catalog has no eager per-chunk footprint inventory.
    #[error("compact provider catalogs do not materialize per-chunk footprints")]
    ProviderFootprintUnavailable,
    /// A payload references a chunk absent from the catalog.
    #[error("chunk {chunk} is absent from the dataset catalog")]
    MissingChunk {
        /// Unknown chunk identity.
        chunk: ChunkId,
    },
    /// A payload does not match its descriptor's declared type.
    #[error("chunk {chunk} expects {expected:?}, received {actual:?}")]
    PayloadKindMismatch {
        /// Addressed chunk.
        chunk: ChunkId,
        /// Descriptor contract.
        expected: PayloadKind,
        /// Supplied payload type.
        actual: PayloadKind,
    },
    /// A payload contains a different number of rows than its descriptor.
    #[error("chunk {chunk} expects {expected} rows, received {actual}")]
    PayloadRowCountMismatch {
        /// Addressed chunk.
        chunk: ChunkId,
        /// Descriptor row count.
        expected: u32,
        /// Payload row count.
        actual: u32,
    },
    /// A typed payload violates its host-side invariants.
    #[error("invalid chunk payload: {reason}")]
    InvalidPayload {
        /// Stable validation reason.
        reason: &'static str,
    },
    /// Computing the minimum retained payload bytes overflowed `u64`.
    #[error("chunk payload byte size exceeds u64")]
    PayloadByteSizeOverflow,
    /// Catalog accounting understates the payload storage retained by a chunk.
    #[error("chunk {chunk} declares {declared} host bytes but needs at least {required}")]
    PayloadFootprintTooSmall {
        /// Chunk whose accounting is insufficient.
        chunk: ChunkId,
        /// Minimum bytes directly retained by the payload.
        required: u64,
        /// Host bytes declared in the catalog.
        declared: u64,
    },
    /// A requested model does not contain a dense coordinate column.
    #[error("structure model has no dense coordinate column")]
    MissingStructureModel,
    /// A structure cannot fit in one chunk-local address space.
    #[error("structure contains more than u32::MAX atoms")]
    StructureTooLarge,
    /// A topology table cannot fit its `u32` offset representation.
    #[error("structure {table} table contains more than u32::MAX rows")]
    StructureTopologyTooLarge {
        /// Topology table whose offsets cannot be represented.
        table: &'static str,
    },
    /// A halo leaves no interior or stored voxel count exceeds `u32`.
    #[error("brick shape is invalid or exceeds chunk-local addressing")]
    InvalidBrickShape,
    /// Scalar or categorical extrema are non-finite or inverted.
    #[error("brick value range is invalid")]
    InvalidBrickRange,
    /// Two sparse descriptors use the same global brick identity.
    #[error("duplicate global brick identity")]
    DuplicateBrick,
    /// Logical extent or mip scaling is invalid.
    #[error("logical volume extent or mip scale is invalid")]
    InvalidLogicalVolume,
    /// A sparse brick interior exceeds the logical volume.
    #[error("brick lies outside the logical volume")]
    BrickOutsideLogicalVolume,
    /// Logical dense byte accounting exceeds `u64`.
    #[error("logical volume byte size exceeds u64")]
    LogicalVolumeOverflow,
    /// A dirty generation cannot advance beyond `u64::MAX`.
    #[error("dirty generation is exhausted")]
    GenerationExhausted,
}
