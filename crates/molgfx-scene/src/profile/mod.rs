//! Small renderer policy values.

mod depth_cue;
mod policy;

pub use depth_cue::DepthCue;
pub use policy::{Quality, RenderProfile, adaptive, highest_fixed, interactive, publication};

#[cfg(test)]
mod tests;
