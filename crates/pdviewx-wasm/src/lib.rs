//! Browser bindings over the same declarative scene and WebGPU engine.

#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(target_arch = "wasm32")]
pub use browser::{WebCamera, WebEngine, WebRepresentation, WebRepresentationKind, WebScene};
