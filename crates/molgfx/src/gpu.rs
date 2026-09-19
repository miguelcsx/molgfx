//! The backend-agnostic GPU abstraction: device, queue, encoder and surface
//! traits, with their descriptors and capability flags.
//!
//! A caller rendering a scene never needs this module. It is published because
//! it is the extension point: a new backend is written against these traits and
//! reaches the engine through them, so the renderer above stays generic over the
//! device with no dynamic dispatch on the hot path.

pub use molgfx_gpu::*;
