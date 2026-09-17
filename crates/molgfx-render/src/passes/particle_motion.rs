//! Fixed-step visual advection for caller-supplied particle samples.

use crate::error::RenderError;
use molgfx_gpu::{
    CommandEncoder as _, ComputePassDesc, ComputePassEncoder as _, ComputePipelineDesc, Device,
    ShaderModuleDesc,
};

const WORKGROUP_SIZE: u32 = 64;

#[derive(Debug)]
pub struct ParticleMotionPass<D: Device> {
    pipeline: D::Pipeline,
}

impl<D: Device> ParticleMotionPass<D> {
    pub fn new(device: &D, primitive: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "fixed-step particle advection",
            wgsl: molgfx_shaders::PARTICLE_ADVECTION,
        })?;
        Ok(Self {
            pipeline: device.create_compute_pipeline(&ComputePipelineDesc {
                label: "fixed-step particle advection",
                layouts: &[None, None, Some(primitive)],
                shader: &shader,
                entry: "advect_particles",
            })?,
        })
    }

    pub(crate) fn record(&self, encoder: &mut D::CommandEncoder, group: &D::BindGroup, count: u32) {
        let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
            label: "fixed-step particle advection",
            timestamps: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(2, group, &[]);
        pass.dispatch(count.div_ceil(WORKGROUP_SIZE), 1, 1);
    }
}
