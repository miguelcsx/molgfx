//! The render graph and the engine that drives a frame.
//!
//! Callers drive rendering entirely through [`Engine`]: a scene, a camera and an
//! image configuration go in, pixels and telemetry come out. Graph internals,
//! passes, pipelines and culling are not exposed.

pub use molgfx_render::*;

/// The engine bound to the backend this build selected.
///
/// The backend is chosen by capability and never named, so this alias is the
/// only `Engine` a caller writes; [`molgfx_render::Engine`] stays generic over
/// the device for code that composes a different one.
pub type Engine = molgfx_render::Engine<molgfx_wgpu::WgpuDevice>;

/// The engine that renders a recorded sequence off-screen.
#[cfg(not(target_arch = "wasm32"))]
pub type SequenceRenderer = molgfx_render::SequenceRenderer<molgfx_wgpu::WgpuDevice>;
