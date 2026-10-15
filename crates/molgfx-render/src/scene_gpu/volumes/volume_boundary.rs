//! Active scalar boundaries share buffers by source identity and isovalue.

use crate::scene_gpu::field_boundary::GpuFieldBoundary;
use molgfx_core::VolumeHandle;
use molgfx_gpu::Device;

#[derive(Debug)]
pub(in crate::scene_gpu) struct VolumeBoundary<D: Device> {
    pub(in crate::scene_gpu) key: (VolumeHandle, u32),
    pub(in crate::scene_gpu) buffers: GpuFieldBoundary<D>,
}
