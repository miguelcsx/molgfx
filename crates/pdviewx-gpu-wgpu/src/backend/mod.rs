//! The wgpu implementation of the backend-neutral GPU contracts.

pub(crate) mod convert;
pub(crate) mod device;
pub(crate) mod encoder;
pub(crate) mod queue;
pub(crate) mod surface;

pub use device::WgpuDevice;
pub use encoder::WgpuCommandEncoder;
pub use queue::WgpuQueue;
pub use surface::WgpuSurface;
