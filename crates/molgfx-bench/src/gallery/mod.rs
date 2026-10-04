//! Canonical gallery contracts and scene recipes shared by measurements and review.

mod catalog;
mod checklist;
mod density;
mod molecular;
mod presentation;
mod scene;
mod segmentation;

pub use catalog::{Camera, Catalog, FitCamera, Fixture, Result, Script, Style, Video, VideoKind};
pub use density::density;
pub use molecular::structure;
pub use presentation::{camera, fit_camera, profile};
pub use scene::{scene, script_scene};
pub use segmentation::segmentation_scene;
