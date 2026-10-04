//! Shared gallery contracts and authored presentation settings.
mod catalog;
mod checklist;
mod presentation;
pub use catalog::{Camera, Catalog, FitCamera, Fixture, Result, Script, Style, Video, VideoKind};
pub use presentation::{camera, fit_camera, profile};
