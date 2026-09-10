//! The render error type.

use thiserror::Error;

/// Everything that can go wrong constructing or driving the engine.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RenderError {
    /// A device-level failure, carried upward with its own code.
    #[error(transparent)]
    Gpu(#[from] pdviewx_gpu::GpuError),

    /// A CPU-built hierarchy exceeded the compact GPU index layout.
    #[error(transparent)]
    Acceleration(#[from] pdviewx_math::BvhBuildError),

    /// A logical entity row cannot be encoded in a chunk-local GPU identity.
    #[error(transparent)]
    Entity(#[from] pdviewx_core::EntityIdError),

    /// A bounded global picking page transition or readback was invalid.
    #[error(transparent)]
    Picking(#[from] pdviewx_core::PickingError),

    /// A chunk span used by a resident pick page was invalid.
    #[error(transparent)]
    Dataset(#[from] pdviewx_core::DatasetError),

    /// A visible scene entity references a structure that is no longer resident.
    #[error("picking entity owner is not resident")]
    PickingOwnerMissing,

    /// A GPU record requested a namespace absent from the bounded page table.
    #[error("dataset {dataset} has no resident {kind:?} picking page")]
    PickingPageMissing {
        /// Dataset whose page was required.
        dataset: pdviewx_core::DatasetId,
        /// Entity namespace whose page was required.
        kind: pdviewx_core::EntityKind,
    },

    /// A validated dynamic relation source is not resident in the GPU scene.
    #[error("dynamic relation source {domain:?} is not resident")]
    RelationSourceMissing {
        /// Spatial table required by the relation stream.
        domain: pdviewx_core::RowDomain,
    },

    /// CPU lowering exceeded a compact GPU record field.
    #[error(transparent)]
    Packing(#[from] pdviewx_geometry::PackingError),

    /// The render graph contains a dependency cycle.
    #[error("render graph cycle involving pass {pass:?}")]
    GraphCycle {
        /// A pass on the cycle.
        pass: &'static str,
    },

    /// A pass referenced a resource id the graph never declared.
    #[error("pass {pass:?} references undeclared resource {resource}")]
    UnknownResource {
        /// The offending pass.
        pass: &'static str,
        /// The undeclared id.
        resource: u32,
    },

    /// Off-screen dimensions overflowed a portable texture/readback layout.
    #[error("invalid off-screen image dimensions")]
    InvalidImageSize,

    /// Sequence timestamps or buffering parameters violate the deterministic contract.
    #[error("invalid render sequence: {reason}")]
    InvalidSequence {
        /// Stable caller-actionable reason.
        reason: &'static str,
    },

    /// The caller must drain a completed sequence frame before submitting more work.
    #[error("render sequence reached its {max_in_flight}-frame in-flight limit")]
    SequenceBackpressure {
        /// Configured bounded readback depth.
        max_in_flight: u8,
    },

    /// A scene-tracked optical target is stale, empty, or behind the camera.
    #[error("invalid optical focus target: {reason}")]
    InvalidFocusTarget {
        /// Stable, caller-actionable reason.
        reason: &'static str,
    },

    /// The mapped image could not be encoded for publication.
    #[error("image encoding failed: {summary}")]
    ImageEncoding {
        /// Encoder-provided diagnostic.
        summary: String,
    },

    /// A render session could not be encoded or decoded.
    #[error("render session encoding failed: {summary}")]
    SessionEncoding {
        /// Serializer-provided diagnostic.
        summary: String,
    },

    /// Fixed residency capacity or lifecycle state rejected frame work.
    #[error("residency operation failed: {reason}")]
    Residency {
        /// Stable failure category without backend-specific text allocation.
        reason: &'static str,
    },

    /// Provider chunk residency failed with a typed lifecycle or capacity error.
    #[error(transparent)]
    ChunkResidency(#[from] crate::engine::ChunkResidencyError),

    /// Sparse volume atlas validation, capacity or lifecycle failure.
    #[error(transparent)]
    BrickAtlas(#[from] crate::scene_gpu::brick_atlas::types::BrickAtlasError),

    /// One caller-owned dataset id was reused for different immutable data.
    #[error(
        "dataset {dataset} identifies conflicting immutable assets ({resident_fingerprint:#018x} != {incoming_fingerprint:#018x})"
    )]
    AssetIdentityCollision {
        /// Caller-owned dataset identity that collided.
        dataset: pdviewx_core::DatasetId,
        /// Fingerprint already resident for the dataset.
        resident_fingerprint: u64,
        /// Fingerprint supplied by the incoming placement.
        incoming_fingerprint: u64,
    },

    /// Sampled-surface component lowering exceeded its bounded working set.
    #[error("surface component filtering failed: {reason}")]
    SurfaceComponents {
        /// Stable validation or capacity reason.
        reason: &'static str,
    },

    /// Immutable GPU storage could not allocate or relocate safely.
    #[error(transparent)]
    AssetArena(#[from] crate::scene_gpu::AssetArenaError),
}

impl RenderError {
    /// The stable registry code for this condition.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Gpu(e) => e.code(),
            Self::Acceleration(_) => "PDVIEWX-E0077",
            Self::Entity(_) => "PDVIEWX-E0078",
            Self::Picking(_) => "PDVIEWX-E0082",
            Self::Dataset(_) => "PDVIEWX-E0083",
            Self::PickingOwnerMissing => "PDVIEWX-E0084",
            Self::PickingPageMissing { .. } => "PDVIEWX-E0085",
            Self::RelationSourceMissing { .. } => "PDVIEWX-E0091",
            Self::Packing(_) => "PDVIEWX-E0079",
            Self::GraphCycle { .. } => "PDVIEWX-E0070",
            Self::UnknownResource { .. } => "PDVIEWX-E0071",
            Self::InvalidImageSize => "PDVIEWX-E0072",
            Self::InvalidSequence { .. } => "PDVIEWX-E0089",
            Self::SequenceBackpressure { .. } => "PDVIEWX-E0090",
            Self::InvalidFocusTarget { .. } => "PDVIEWX-E0073",
            Self::ImageEncoding { .. } => "PDVIEWX-E0074",
            Self::SessionEncoding { .. } => "PDVIEWX-E0075",
            Self::Residency { .. } => "PDVIEWX-E0076",
            Self::ChunkResidency(_) => "PDVIEWX-E0086",
            Self::BrickAtlas(_) => "PDVIEWX-E0088",
            Self::AssetIdentityCollision { .. } => "PDVIEWX-E0080",
            Self::SurfaceComponents { .. } => "PDVIEWX-E0087",
            Self::AssetArena(_) => "PDVIEWX-E0081",
        }
    }
}
