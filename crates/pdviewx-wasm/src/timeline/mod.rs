//! Batch-only browser timeline adapters for generic scene data.

mod controller;
mod model;
mod transforms;

pub use controller::WebTimeline;
pub use model::{WebPlaybackMode, WebTimeWarp, WebTimelineTrackHandle};
