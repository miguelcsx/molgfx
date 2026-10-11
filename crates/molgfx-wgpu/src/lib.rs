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
    WgpuBuffer, WgpuCommandEncoder, WgpuDevice, WgpuQueue, WgpuReadback, WgpuShaderModule,
    WgpuSurface, WgpuTexture,
};

/// The wgpu version the host-interop types are expressed in. A host must use
/// this exact version (or re-export it) so its handles are the same types.
#[cfg(feature = "host")]
pub use backend::{
    HostContext, HostRequirements, host_device_descriptor, host_requirements, unmet_requirements,
};
#[cfg(feature = "host")]
pub use wgpu;
