//! Browser bindings for `MolGFX`'s versioned semantic scene protocol.

#![forbid(unsafe_code)]

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod contract;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use contract::{WebRenderer, WebScene, WebScenePatch, WebSceneSpec};
