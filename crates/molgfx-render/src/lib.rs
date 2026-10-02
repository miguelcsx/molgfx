//! The render graph, passes and the engine that drives a frame.
//!
//! Public surface: the engine, its configuration, render targets and typed
//! outcomes. Graph internals, passes, pipelines and culling are not exposed;
//! callers drive rendering entirely through [`Engine`].

#![forbid(unsafe_code)]

mod engine;
mod error;
mod graph;
mod passes;
mod residency;
mod residency_machine;
mod scene_gpu;

pub(crate) use molgfx_core::fallback;

#[cfg(test)]
mod testing;

pub use engine::{
    AdaptiveQuality, AdaptiveQualityConfig, AntiAliasingStyle, AttributeChunkWindow, BackdropStyle,
    BloomStyle, BondChunkPlacement, ChunkPlacementError, ChunkPlacementId, ChunkPlacementStatus,
    ChunkRepresentation, ChunkResidencyError, ChunkResidencyMetrics,
    DEFAULT_SURFACE_FIELD_BUDGET_BYTES, DepthCue, DepthOfField, DerivedCacheBudget,
    DerivedCacheUsage, DisplayGamut, DisplayTransform, EffectLayer, Engine, EngineConfig,
    FocusTarget, FrameCompleteness, FrameDegradation, FrameMetrics, FrameReport, FrameStatus,
    FrameTiming, GpuTiming, HdrImage, IllustrationStyle, Image, ImageConfig,
    InstanceChunkPlacement, InstanceChunkWindow, LigandPoseStats, LightingEnvironment, MotionBlur,
    PICK_READBACK_BYTES, PendingPick, Pick, PickEntity, PointChunkPlacement, PresentationEffect,
    QualityTier, RelationChunkPlacement, RenderMode, RenderProfile, RenderSession,
    ResidentGenericChunk, ResidentStructureChunk, ResidentTrajectoryChunk, ResolvedRenderPlan,
    StructureChunkPlacement, ToneMapping, TrajectoryChunkWindow, TransferFunction,
};
pub use engine::{CompletedFrame, CpuStages, EffectiveQuality, SurfaceLimit};
pub(crate) use engine::{
    DerivedCache, DerivedCacheClass, DerivedCacheKey, DerivedFootprint, MaterializationPlan,
};
#[cfg(not(target_arch = "wasm32"))]
pub use engine::{FrameTicket, SequenceConfig, SequenceExposure, SequenceFrame, SequenceRenderer};
pub use engine::{PassTiming, PassTimingCoverage};
pub use error::RenderError;
pub use molgfx_geometry::PackingError;
pub use residency::{
    ImmutableArenaMetrics, ResidencyConfig, ResidencyCounters, ResidencyInitError,
    ResidencyMetrics, ResidencyWorkspace,
};
pub use residency_machine::{
    ResidencyMachine, ResidencyMachineError, ResidencyMachineMetrics, ResidencyState,
    ResidencyTicket,
};
pub use scene_gpu::brick_atlas::types::{
    BrickAtlasConfig, BrickAtlasError, BrickAtlasKind, BrickAtlasMetrics, BrickAtlasPoll,
    BrickAtlasUpload,
};
pub use scene_gpu::brick_atlas::upload::GpuBrickAtlas;
