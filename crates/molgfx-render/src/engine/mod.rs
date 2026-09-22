//! The engine: owns the device, the graph and every pass's state.

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
mod derived_cache;
mod exr;
mod focus_target;
mod frame;
mod graph_setup;
mod graph_transparency;
mod hdr_image;
mod image;
mod init;
mod lighting_environment;
mod picking;
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
    EngineConfig, FrameCompleteness, FrameDegradation, FrameMetrics, FrameReport, FrameStatus,
    RenderMode,
};
pub use derived_cache::{DerivedCacheBudget, DerivedCacheUsage};
pub use hdr_image::HdrImage;
pub use image::{Image, ImageConfig};
pub use init::Engine;
pub use lighting_environment::LightingEnvironment;
pub use picking::{Pick, PickEntity};
pub use profile::{
    BloomStyle, DepthOfField, EffectLayer, FocusTarget, IllustrationStyle, MotionBlur,
    PresentationEffect, RenderProfile, ResolvedRenderPlan,
};
pub use profiling::FrameTiming;
#[cfg(not(target_arch = "wasm32"))]
pub use sequence::{FrameTicket, SequenceConfig, SequenceFrame, SequenceRenderer};
pub use session::RenderSession;
pub use statistics::LigandPoseStats;

pub(crate) use crate::passes::PassRegistry;
pub(crate) use derived_cache::{
    DerivedCache, DerivedCacheKey, DerivedFootprint, MaterializationPlan,
};
pub(crate) use focus_target::FocusTracker;
pub(crate) use picking::Picker;
pub(crate) use profiling::GpuProfiler;
pub(crate) use shadow::ShadowBoundCache;
pub(crate) use temporal::{TemporalOptions, TemporalState};

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "engine_tests.rs"]
mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod slot_cache_tests;

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
