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

#[cfg(test)]
mod testing;

pub use engine::{
    AttributeChunkWindow, BackdropStyle, BloomStyle, BondChunkPlacement, ChunkPlacementError,
    ChunkPlacementId, ChunkPlacementStatus, ChunkRepresentation, ChunkResidencyError,
    ChunkResidencyMetrics, DepthOfField, DerivedCacheBudget, DerivedCacheUsage, DisplayGamut,
    DisplayTransform, EffectLayer, Engine, EngineConfig, FocusTarget, FrameCompleteness,
    FrameDegradation, FrameMetrics, FrameReport, FrameStatus, FrameTiming, HdrImage,
    IllustrationStyle, Image, ImageConfig, InstanceChunkPlacement, InstanceChunkWindow,
    LigandPoseStats, LightingEnvironment, MotionBlur, Pick, PickEntity, PointChunkPlacement,
    PresentationEffect, RelationChunkPlacement, RenderMode, RenderProfile, RenderSession,
    ResidentGenericChunk, ResidentStructureChunk, ResidentTrajectoryChunk, ResolvedRenderPlan,
    StructureChunkPlacement, ToneMapping, TrajectoryChunkWindow, TransferFunction,
};
pub(crate) use engine::{DerivedCache, DerivedFootprint, MaterializationPlan};
#[cfg(not(target_arch = "wasm32"))]
pub use engine::{FrameTicket, SequenceConfig, SequenceFrame, SequenceRenderer};
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
