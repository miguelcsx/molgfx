//! The render error type.

use thiserror::Error;

/// Everything that can go wrong constructing or driving the engine.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RenderError {
    /// A device-level failure, carried upward with its own code.
    #[error(transparent)]
    Gpu(#[from] molgfx_gpu::GpuError),

    /// A CPU-built hierarchy exceeded the compact GPU index layout.
    #[error(transparent)]
    Acceleration(#[from] molgfx_math::BvhBuildError),

    /// A logical entity row cannot be encoded in a chunk-local GPU identity.
    #[error(transparent)]
    Entity(#[from] molgfx_core::EntityIdError),

    /// A bounded global picking page transition or readback was invalid.
    #[error(transparent)]
    Picking(#[from] molgfx_core::PickingError),

    /// A chunk span used by a resident pick page was invalid.
    #[error(transparent)]
    Dataset(#[from] molgfx_core::DatasetError),

    /// A visible scene entity references a structure that is no longer resident.
    #[error("picking entity owner is not resident")]
    PickingOwnerMissing,

    /// A GPU record requested a namespace absent from the bounded page table.
    #[error("dataset {dataset} has no resident {kind:?} picking page")]
    PickingPageMissing {
        /// Dataset whose page was required.
        dataset: molgfx_core::DatasetId,
        /// Entity namespace whose page was required.
        kind: molgfx_core::EntityKind,
    },

    /// A validated dynamic relation source is not resident in the GPU scene.
    #[error("dynamic relation source {domain:?} is not resident")]
    RelationSourceMissing {
        /// Spatial table required by the relation stream.
        domain: molgfx_core::RowDomain,
    },

    /// CPU lowering exceeded a compact GPU record field.
    #[error(transparent)]
    Packing(#[from] molgfx_geometry::PackingError),

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
        dataset: molgfx_core::DatasetId,
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
            Self::Acceleration(_) => "MOLGFX-E0077",
            Self::Entity(_) => "MOLGFX-E0078",
            Self::Picking(_) => "MOLGFX-E0082",
            Self::Dataset(_) => "MOLGFX-E0083",
            Self::PickingOwnerMissing => "MOLGFX-E0084",
            Self::PickingPageMissing { .. } => "MOLGFX-E0085",
            Self::RelationSourceMissing { .. } => "MOLGFX-E0091",
            Self::Packing(_) => "MOLGFX-E0079",
            Self::GraphCycle { .. } => "MOLGFX-E0070",
            Self::UnknownResource { .. } => "MOLGFX-E0071",
            Self::InvalidImageSize => "MOLGFX-E0072",
            Self::InvalidSequence { .. } => "MOLGFX-E0089",
            Self::SequenceBackpressure { .. } => "MOLGFX-E0090",
            Self::InvalidFocusTarget { .. } => "MOLGFX-E0073",
            Self::ImageEncoding { .. } => "MOLGFX-E0074",
            Self::SessionEncoding { .. } => "MOLGFX-E0075",
            Self::Residency { .. } => "MOLGFX-E0076",
            Self::ChunkResidency(_) => "MOLGFX-E0086",
            Self::BrickAtlas(_) => "MOLGFX-E0088",
            Self::AssetIdentityCollision { .. } => "MOLGFX-E0080",
            Self::SurfaceComponents { .. } => "MOLGFX-E0087",
            Self::AssetArena(_) => "MOLGFX-E0081",
        }
    }
}
