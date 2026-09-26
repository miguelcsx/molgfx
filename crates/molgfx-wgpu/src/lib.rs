//! The wgpu implementation of the backend-neutral GPU contracts.
//!
//! Safe Rust throughout: wgpu owns every low-level memory boundary. This crate is
//! selected by the engine through capability, never named by callers; its
//! only public item is the device type the facade aliases.

#![forbid(unsafe_code)]

mod backend;

pub(crate) use backend::{convert, device, encoder, queue, surface};

#[cfg(not(target_arch = "wasm32"))]
pub use backend::{AdapterReport, SystemInfo, system_info};
pub use backend::{
    WgpuBuffer, WgpuCommandEncoder, WgpuDevice, WgpuQueue, WgpuSurface, WgpuTexture,
};
