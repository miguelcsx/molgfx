//! Small renderer policy values.

mod depth_cue;
mod policy;

pub use depth_cue::DepthCue;
pub use policy::{Quality, RenderProfile, adaptive, converged, highest_fixed, interactive};

#[cfg(test)]
mod tests;
