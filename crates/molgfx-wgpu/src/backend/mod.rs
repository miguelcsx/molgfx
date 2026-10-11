//! The wgpu implementation of the backend-neutral GPU contracts.

pub(crate) mod convert;
pub(crate) mod device;
pub(crate) mod device_caps;
pub(crate) mod device_errors;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod diagnostics;
pub(crate) mod encoder;
#[cfg(feature = "host")]
pub(crate) mod host;
pub(crate) mod queue;
pub(crate) mod ray_query;
pub(crate) mod resource;
pub(crate) mod stage_constants;
pub(crate) mod surface;

pub use device::WgpuDevice;
#[cfg(not(target_arch = "wasm32"))]
pub use diagnostics::{AdapterReport, SystemInfo, system_info};
pub use encoder::WgpuCommandEncoder;
#[cfg(feature = "host")]
pub use host::{
    HostContext, HostRequirements, host_device_descriptor, host_requirements, unmet_requirements,
};
pub use queue::{WgpuQueue, WgpuReadback};
pub use resource::{WgpuBuffer, WgpuTexture};
pub use stage_constants::WgpuShaderModule;
pub use surface::WgpuSurface;
