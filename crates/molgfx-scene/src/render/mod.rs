//! Physical renderer wrapper with target-sized rendering.

pub use molgfx_render::{
    CompletedFrame, CpuStages, EffectiveQuality, FrameReport, FrameTiming, GpuTiming,
    MeasuredOutput, PassTiming, PassTimingCoverage, PollOutcome, QualityTier, SurfaceLimit,
};

mod device_loss;
pub use device_loss::DeviceLossReport;
mod engine_config;
#[cfg(feature = "host")]
mod host;
#[cfg(feature = "host")]
pub use host::{HostFrame, HostOutputFormat, HostTarget};
mod pick;
pub use pick::PickReadback;

#[cfg(not(target_arch = "wasm32"))]
mod hdr_image;
#[cfg(not(target_arch = "wasm32"))]
mod image;
#[cfg(not(target_arch = "wasm32"))]
mod stereo;
#[cfg(not(target_arch = "wasm32"))]
pub use stereo::{StereoImages, StereoLayout};
mod pick_result;
mod point_cloud;
mod renderer;
mod timeline_sequence;
pub use timeline_sequence::{FrameSchedule, SequenceManifest};
#[cfg(test)]
mod timeline_fixture_tests;
#[cfg(not(target_arch = "wasm32"))]
mod timeline_render;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod timeline_render_tests;
pub use point_cloud::PointCloud;

#[cfg(not(target_arch = "wasm32"))]
pub use hdr_image::HdrImage;
#[cfg(not(target_arch = "wasm32"))]
pub use image::Image;
pub use pick_result::{PickKind, PickResult};
pub use renderer::Renderer;
