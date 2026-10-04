//! The engine: owns the device, the graph and every pass's state.

mod adaptive;
mod backdrop;
pub(crate) mod bond_draw_plan;
mod bond_residency;
mod brick_atlas;
mod chunk_api;
pub(crate) mod chunk_draw_plan;
mod chunk_placement;
mod chunk_residency;
mod chunk_residency_support;
mod chunk_residency_types;
mod config;
mod cpu_stages;
mod depth_cue;
mod derived_cache;
mod effective_quality;
mod exposure;
mod exr;
mod focus_target;
mod frame;
mod gpu_timing;
mod graph_setup;
mod graph_transparency;
mod hdr_image;
mod image;
mod init;
mod lighting_environment;
mod optics;
mod pass_profiling;
mod picking;
pub(crate) mod pipeline_cache;
mod presentation;
mod profile;
mod profile_numeric;
mod profiling;
#[cfg(not(target_arch = "wasm32"))]
mod sequence;
mod session;
mod settings;
mod shadow;
mod statistics;
mod target;
mod temporal;

pub(crate) use adaptive::occlusion_rays;
pub use adaptive::{AdaptiveQuality, AdaptiveQualityConfig, QualityTier};
pub use backdrop::{BackdropStyle, DisplayGamut, DisplayTransform, ToneMapping, TransferFunction};
pub use chunk_placement::{
    AttributeChunkWindow, BondChunkPlacement, ChunkPlacementError, ChunkPlacementId,
    ChunkPlacementStatus, ChunkRepresentation, InstanceChunkPlacement, InstanceChunkWindow,
    PointChunkPlacement, RelationChunkPlacement, StructureChunkPlacement, TrajectoryChunkWindow,
};
pub use chunk_residency_types::{
    ChunkResidencyError, ChunkResidencyMetrics, ResidentGenericChunk, ResidentStructureChunk,
    ResidentTrajectoryChunk,
};
pub use config::{
    CompletedFrame, DEFAULT_SURFACE_FIELD_BUDGET_BYTES, EngineConfig, FrameCompleteness,
    FrameDegradation, FrameMetrics, FrameReport, FrameStatus, RenderMode,
};
pub use cpu_stages::CpuStages;
pub use depth_cue::DepthCue;
pub use derived_cache::{DerivedCacheBudget, DerivedCacheUsage};
pub use effective_quality::{EffectiveQuality, SurfaceLimit};
pub use gpu_timing::GpuTiming;
pub use hdr_image::HdrImage;
pub use image::{Image, ImageConfig};
pub use init::Engine;
pub use lighting_environment::LightingEnvironment;
pub use optics::{DepthOfField, FocusTarget, MotionBlur};
pub use pass_profiling::{PassTiming, PassTimingCoverage};
pub use picking::{PICK_READBACK_BYTES, PendingPick, Pick, PickEntity};
pub use profile::{
    AntiAliasingStyle, BloomStyle, EffectLayer, PresentationEffect, RenderProfile,
    ResolvedRenderPlan, ShapeCueStyle,
};
pub use profiling::{FrameTiming, MeasuredOutput};
#[cfg(not(target_arch = "wasm32"))]
pub use sequence::{
    FrameTicket, SequenceConfig, SequenceExposure, SequenceFrame, SequenceRenderer,
};
pub use session::RenderSession;
pub use statistics::LigandPoseStats;

pub(crate) use crate::passes::PassRegistry;
pub(crate) use derived_cache::{
    DerivedCache, DerivedCacheClass, DerivedCacheKey, DerivedFootprint, MaterializationPlan,
};
pub(crate) use focus_target::FocusTracker;
pub(crate) use picking::Picker;
pub(crate) use profiling::GpuProfiler;
pub(crate) use shadow::ShadowBoundCache;
pub(crate) use temporal::{TemporalOptions, TemporalState};

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "engine_tests.rs"]
pub(crate) mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "exposure_tests.rs"]
mod exposure_tests;

#[cfg(test)]
#[path = "pipeline_cache_tests.rs"]
mod pipeline_cache_tests;

#[cfg(test)]
#[path = "indirect_arena_tests.rs"]
mod indirect_arena_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod slot_cache_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod surface_field_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod color_scheme_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "residency_integration_tests.rs"]
mod residency_integration_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "chunk_residency_tests.rs"]
mod chunk_residency_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "chunk_residency/generic_tests.rs"]
mod generic_chunk_residency_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "chunk_residency/generic_visual_tests.rs"]
mod generic_chunk_visual_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "chunk_residency/instance_timeline_tests.rs"]
mod chunk_instance_timeline_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "chunk_render_tests.rs"]
mod chunk_render_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "chunk_bond_render_tests.rs"]
mod chunk_bond_render_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "asset_sharing_tests.rs"]
mod asset_sharing_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "scene_identity_tests.rs"]
mod scene_identity_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "draw_tests.rs"]
mod draw_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "generic_instance_tests.rs"]
mod generic_instance_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "generic_point_tests.rs"]
mod generic_point_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "generic_relation_tests.rs"]
mod generic_relation_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "generic_timeline_budget_tests.rs"]
mod generic_timeline_budget_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "ligand_pose_tests.rs"]
mod ligand_pose_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "putty_tests.rs"]
mod putty_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "async_tests.rs"]
mod async_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "picking_tests.rs"]
mod picking_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "scalar_overlay_tests.rs"]
mod scalar_overlay_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "surface_component_tests.rs"]
mod surface_component_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "surface_memory_tests.rs"]
mod surface_memory_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "interaction_tests.rs"]
mod interaction_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "label_tests.rs"]
mod label_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "property_tests.rs"]
mod property_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "trajectory_tests.rs"]
mod trajectory_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "volume_tests.rs"]
mod volume_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod segmentation_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod completion_tests;
