//! Small renderer policy values.

mod depth_cue;
mod effect;
mod policy;

pub use depth_cue::DepthCue;
pub use effect::{AntiAliasing, Effect, EffectKind, EffectSet};
pub use effect::{
    BackdropStyle, BloomStyle, DepthOfField, DisplayTransform, FocusTarget, LightingEnvironment,
    MotionBlur, ShapeCueStyle,
};
pub use molgfx_math::Rgba8;
pub use policy::{Quality, RenderProfile, adaptive, converged, highest_fixed, interactive};

#[cfg(test)]
mod tests;
