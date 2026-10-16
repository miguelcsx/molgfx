//! The engine: owns the device, the graph and every pass's state.

mod bonds;
pub(crate) use bonds::bond_draw_plan;
use bonds::bond_residency;

mod chunk;
pub(crate) use chunk::chunk_draw_plan;
use chunk::chunk_placement;
use chunk::chunk_residency;
use chunk::chunk_residency_support;
use chunk::chunk_residency_types;
use chunk::derived_cache;

mod graph;
use graph::graph_setup;
use graph::graph_transparency;
pub(crate) use graph::pipeline_cache;
use graph::target;

mod history;
use history::temporal;
use history::temporal_scene;

mod lighting;
use lighting::lighting_environment;
use lighting::shadow;

mod output;
use output::exposure;
use output::exr;
use output::hdr_image;
use output::host;
use output::image;
#[cfg(not(target_arch = "wasm32"))]
use output::sequence;
use output::stereo;

mod quality;
use quality::adaptive;
use quality::effective_quality;

mod runtime;
use runtime::config;
use runtime::init;
use runtime::session;

mod style;
use style::backdrop;
use style::profile;
use style::profile_numeric;

mod timing;
use timing::cpu_stages;
use timing::gpu_timing;
use timing::pass_profiling;
use timing::profiling;
use timing::statistics;

mod view;
use view::depth_cue;
use view::focus_target;
use view::optics;
use view::pick_stream;
use view::picking;

pub use stereo::{StereoImages, StereoLayout};

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
pub use host::{HostFrame, HostTarget};
pub use image::{Image, ImageConfig};
pub use init::Engine;
pub use lighting_environment::LightingEnvironment;
pub use optics::{DepthOfField, FocusTarget, MotionBlur};
pub use pass_profiling::{PassTiming, PassTimingCoverage};
pub use pick_stream::{PickPolicy, PickStream, PickUpdate, PollReport};
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

#[cfg(test)]
mod integration;
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) use integration::tests;
