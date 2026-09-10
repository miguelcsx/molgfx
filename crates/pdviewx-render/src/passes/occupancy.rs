//! GPU-resident temporal occupancy accumulation pipelines.

use crate::error::RenderError;
use pdviewx_gpu::{ComputePipelineDesc, Device, ShaderModuleDesc};

#[derive(Debug)]
pub struct OccupancyPass<D: Device> {
    pub(crate) clear: D::Pipeline,
    pub(crate) decay: D::Pipeline,
    pub(crate) deposit: D::Pipeline,
    pub(crate) resolve: D::Pipeline,
    pub(crate) bounds: D::Pipeline,
}

impl<D: Device> OccupancyPass<D> {
    pub fn new(device: &D, layout: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "temporal occupancy accumulation",
            wgsl: pdviewx_shaders::OCCUPANCY,
        })?;
        let pipeline = |label, entry| {
            device.create_compute_pipeline(&ComputePipelineDesc {
                label,
                layouts: &[Some(layout)],
                shader: &shader,
                entry,
            })
        };
        Ok(Self {
            clear: pipeline("clear temporal occupancy", "clear_occupancy")?,
            decay: pipeline("decay temporal occupancy", "decay_occupancy")?,
            deposit: pipeline("deposit temporal occupancy", "deposit_occupancy")?,
            resolve: pipeline("resolve temporal occupancy", "resolve_occupancy")?,
            bounds: pipeline(
                "resolve temporal occupancy bounds",
                "resolve_occupancy_bounds",
            )?,
        })
    }
}
