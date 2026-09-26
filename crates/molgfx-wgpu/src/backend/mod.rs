//! The wgpu implementation of the backend-neutral GPU contracts.

pub(crate) mod convert;
pub(crate) mod device;
pub(crate) mod device_caps;
pub(crate) mod device_errors;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod diagnostics;
pub(crate) mod encoder;
pub(crate) mod queue;
pub(crate) mod ray_query;
pub(crate) mod resource;
pub(crate) mod surface;

pub use device::WgpuDevice;
#[cfg(not(target_arch = "wasm32"))]
pub use diagnostics::{AdapterReport, SystemInfo, system_info};
pub use encoder::WgpuCommandEncoder;
pub use queue::WgpuQueue;
pub use resource::{WgpuBuffer, WgpuTexture};
pub use surface::WgpuSurface;
