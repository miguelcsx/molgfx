//! GPU interpolation of caller-supplied topology-aligned frames.

use crate::error::RenderError;
use pdviewx_gpu::{ComputePipelineDesc, Device, ShaderModuleDesc};

#[derive(Debug)]
pub struct TrajectoryPass<D: Device> {
    pub(crate) pipeline: D::Pipeline,
}

impl<D: Device> TrajectoryPass<D> {
    pub fn new(device: &D, layout: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "trajectory interpolation",
            wgsl: pdviewx_shaders::TRAJECTORY,
        })?;
        Ok(Self {
            pipeline: device.create_compute_pipeline(&ComputePipelineDesc {
                label: "trajectory interpolation",
                layouts: &[Some(layout)],
                shader: &shader,
                entry: "interpolate_trajectory",
            })?,
        })
    }
}
