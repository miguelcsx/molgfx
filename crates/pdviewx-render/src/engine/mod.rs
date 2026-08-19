//! The engine: owns the device, the graph and every pass's state.

mod backdrop;
mod config;
mod focus_target;
mod frame;
mod graph_setup;
mod graph_transparency;
mod image;
mod init;
mod lighting_environment;
mod picking;
mod profile;
mod profiling;
mod session;
mod settings;
mod shadow;
mod target;
mod temporal;

pub use backdrop::{BackdropStyle, DisplayGamut, DisplayTransform, ToneMapping, TransferFunction};
pub use config::{EngineConfig, FrameOutcome, RenderMode};
pub use image::{Image, ImageConfig};
pub use init::Engine;
pub use lighting_environment::LightingEnvironment;
pub use picking::{Pick, PickEntity};
pub use profile::{
    BloomStyle, DepthOfField, EffectLayer, FocusTarget, IllustrationStyle, MotionBlur,
    PresentationEffect, RenderProfile, ResolvedRenderPlan,
};
pub use profiling::FrameTiming;
pub use session::RenderSession;

pub(crate) use crate::passes::PassRegistry;
pub(crate) use focus_target::FocusTracker;
pub(crate) use picking::Picker;
pub(crate) use profiling::GpuProfiler;
pub(crate) use shadow::fit as fit_shadow;
pub(crate) use temporal::{TemporalOptions, TemporalState};

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "scene_identity_tests.rs"]
mod scene_identity_tests;

#[cfg(test)]
#[path = "draw_tests.rs"]
mod draw_tests;

#[cfg(test)]
#[path = "async_tests.rs"]
mod async_tests;

#[cfg(test)]
#[path = "scalar_overlay_tests.rs"]
mod scalar_overlay_tests;

#[cfg(test)]
#[path = "interaction_tests.rs"]
mod interaction_tests;

#[cfg(test)]
#[path = "label_tests.rs"]
mod label_tests;

#[cfg(test)]
#[path = "property_tests.rs"]
mod property_tests;

#[cfg(test)]
#[path = "trajectory_tests.rs"]
mod trajectory_tests;

#[cfg(test)]
#[path = "volume_tests.rs"]
mod volume_tests;
