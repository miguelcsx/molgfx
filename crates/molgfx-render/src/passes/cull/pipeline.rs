// Shared compute shader and pipeline construction.

use crate::error::RenderError;
use molgfx_gpu::{ComputePipelineDesc, Device, ShaderModuleDesc};

pub(super) fn shader<D: Device>(
    device: &D,
    label: &'static str,
    wgsl: &'static str,
) -> Result<D::ShaderModule, RenderError> {
    device
        .create_shader_module(&ShaderModuleDesc { label, wgsl })
        .map_err(Into::into)
}

pub(super) fn pipeline<D: Device>(
    device: &D,
    shader: &D::ShaderModule,
    layouts: &[Option<&D::BindGroupLayout>],
    label: &'static str,
    entry: &'static str,
) -> Result<D::Pipeline, RenderError> {
    device
        .create_compute_pipeline(&ComputePipelineDesc {
            label,
            layouts,
            shader,
            entry,
        })
        .map_err(Into::into)
}
