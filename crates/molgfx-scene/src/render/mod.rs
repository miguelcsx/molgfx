//! Physical renderer wrapper with target-sized rendering.

pub use molgfx_render::{
    CompletedFrame, CpuStages, EffectiveQuality, FrameReport, FrameTiming, GpuTiming,
    MeasuredOutput, PassTiming, PassTimingCoverage, QualityTier, SurfaceLimit,
};

mod engine_config;
mod pick;
pub use pick::PickReadback;

#[cfg(not(target_arch = "wasm32"))]
mod hdr_image;
#[cfg(not(target_arch = "wasm32"))]
mod image;
mod pick_result;
mod point_cloud;
mod renderer;
pub use point_cloud::PointCloud;

#[cfg(not(target_arch = "wasm32"))]
pub use hdr_image::HdrImage;
#[cfg(not(target_arch = "wasm32"))]
pub use image::Image;
pub use pick_result::{PickKind, PickResult};
pub use renderer::Renderer;
