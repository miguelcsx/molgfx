//! Physical renderer wrapper with target-sized rendering.

pub use molgfx_render::{
    CompletedFrame, CpuStages, EffectiveQuality, FrameReport, FrameTiming, GpuTiming, PassTiming,
    PassTimingCoverage, QualityTier, SurfaceLimit,
};

mod engine_config;
mod pick;
pub use pick::PickReadback;

#[cfg(not(target_arch = "wasm32"))]
mod image;
mod pick_result;
mod renderer;

#[cfg(not(target_arch = "wasm32"))]
pub use image::Image;
pub use pick_result::{PickKind, PickResult};
pub use renderer::Renderer;
