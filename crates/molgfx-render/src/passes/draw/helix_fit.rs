//! One linear-cost guide fit per helix, before all ribbon drawing passes.
use crate::RenderError;
use molgfx_gpu::{ComputePipelineDesc, Device, ShaderModuleDesc};

#[derive(Debug)]
pub(crate) struct HelixFitPass<D: Device> {
    pub(crate) pipeline: D::Pipeline,
}

impl<D: Device> HelixFitPass<D> {
    pub(crate) fn new(device: &D, layout: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "live helix fitting",
            wgsl: molgfx_shaders::HELIX_FIT,
        })?;
        Ok(Self {
            pipeline: device.create_compute_pipeline(&ComputePipelineDesc {
                label: "live helix fitting",
                layouts: &[Some(layout)],
                shader: &shader,
                entry: "fit_helix",
            })?,
        })
    }
}
