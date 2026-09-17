//! Browser bindings over the same declarative scene and WebGPU engine.

#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
mod browser_camera;
#[cfg(target_arch = "wasm32")]
mod browser_controls;
#[cfg(target_arch = "wasm32")]
mod browser_engine;
#[cfg(target_arch = "wasm32")]
mod browser_frame;
#[cfg(target_arch = "wasm32")]
mod browser_presentation;
#[cfg(target_arch = "wasm32")]
mod browser_secondary;
#[cfg(target_arch = "wasm32")]
mod browser_types;
#[cfg(target_arch = "wasm32")]
mod generated_out_of_core;
#[cfg(target_arch = "wasm32")]
mod generated_surface;
#[cfg(target_arch = "wasm32")]
mod generic_batches;
#[cfg(target_arch = "wasm32")]
mod generic_compositions;
#[cfg(target_arch = "wasm32")]
mod timeline;

#[cfg(target_arch = "wasm32")]
pub use generated_surface::*;
