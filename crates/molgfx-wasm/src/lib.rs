//! Browser bindings for `MolGFX`'s versioned semantic scene protocol.

#![forbid(unsafe_code)]

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod camera;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod contract;

pub mod paging;

pub mod timeline_wire;

#[cfg(test)]
mod timeline_wire_tests;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod timeline;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod camera_path;

#[cfg(test)]
mod paging_tests;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod quality;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod renderer;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod session;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use camera::{WebArcball, WebCamera, WebFly, WebOrbit};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use session::WebSession;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use contract::{WebScene, WebScenePatch, WebSceneSpec};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use renderer::{WebPickReadback, WebRenderer};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use camera_path::{WebCameraBookmark, WebCameraPath, WebCameraPathBuilder};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use timeline::{WebTimeline, WebTrackReading};
