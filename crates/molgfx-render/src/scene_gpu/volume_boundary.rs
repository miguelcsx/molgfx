//! Active scalar boundaries share buffers by source identity and isovalue.

use super::field_boundary::GpuFieldBoundary;
use molgfx_core::VolumeHandle;
use molgfx_gpu::Device;

#[derive(Debug)]
pub(super) struct VolumeBoundary<D: Device> {
    pub(super) key: (VolumeHandle, u32),
    pub(super) buffers: GpuFieldBoundary<D>,
}
