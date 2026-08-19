//! Persistent rolling-probe solvent-excluded field generation.

use crate::error::RenderError;
use pdviewx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, CommandEncoder as _, ComputePassDesc,
    ComputePassEncoder as _, ComputePipelineDesc, Device, ShaderModuleDesc, ShaderStages,
};

#[cfg(test)]
#[path = "surface_field_tests.rs"]
mod tests;

const WORKGROUP_EDGE: u32 = 4;

/// Compute state shared by every SES representation.
#[derive(Debug)]
pub struct SurfaceFieldPass<D: Device> {
    generate_union: D::Pipeline,
    generate_gaussian: D::Pipeline,
    erode: D::Pipeline,
}

impl<D: Device> SurfaceFieldPass<D> {
    pub fn output_layout(device: &D) -> D::BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group1: surface field output",
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture3dWrite {
                        format: pdviewx_gpu::TextureFormat::R32Float,
                    },
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture3dWrite {
                        format: pdviewx_gpu::TextureFormat::R32Uint,
                    },
                },
            ],
        })
    }

    pub fn input_layout(device: &D) -> D::BindGroupLayout {
        let storage = |binding| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Storage { read_only: true },
        };
        device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group2: surface field input",
            entries: &[
                storage(0),
                storage(1),
                storage(6),
                storage(7),
                storage(8),
                BindGroupLayoutEntry {
                    binding: 9,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Uniform,
                },
            ],
        })
    }

    pub fn erosion_layout(device: &D) -> D::BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group1: surface field erosion",
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture3dFloat { filterable: false },
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture3dWrite {
                        format: pdviewx_gpu::TextureFormat::R32Float,
                    },
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture3dUint,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture3dWrite {
                        format: pdviewx_gpu::TextureFormat::R32Uint,
                    },
                },
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Storage { read_only: true },
                },
            ],
        })
    }

    pub fn new(
        device: &D,
        output: &D::BindGroupLayout,
        erosion: &D::BindGroupLayout,
        representation: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "rolling-probe surface field",
            wgsl: pdviewx_shaders::SURFACE_FIELD_COMPUTE,
        })?;
        let erosion_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "rolling-probe surface erosion",
            wgsl: pdviewx_shaders::SURFACE_FIELD_ERODE,
        })?;
        Ok(Self {
            // The probe-inflated union and the Gaussian density sum are
            // different fields, not two settings of one: the first is a
            // distance to inflated atomic spheres, the second a sum of
            // atom-centred densities. Each gets its own pipeline so neither
            // can silently stand in for the other.
            generate_union: device.create_compute_pipeline(&ComputePipelineDesc {
                label: "probe-inflated surface field",
                layouts: &[None, Some(output), Some(representation)],
                shader: &shader,
                entry: "cs_surface_field_union",
            })?,
            generate_gaussian: device.create_compute_pipeline(&ComputePipelineDesc {
                label: "gaussian density field",
                layouts: &[None, Some(output), Some(representation)],
                shader: &shader,
                entry: "cs_surface_field_gaussian",
            })?,
            erode: device.create_compute_pipeline(&ComputePipelineDesc {
                label: "rolling-probe surface erosion",
                layouts: &[None, Some(erosion), Some(representation)],
                shader: &erosion_shader,
                entry: "cs_surface_field_erode",
            })?,
        })
    }

    pub(crate) fn record_generate(
        &self,
        encoder: &mut D::CommandEncoder,
        output: &D::BindGroup,
        representation: &D::BindGroup,
        dimensions: [u32; 3],
        gaussian: bool,
    ) {
        let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
            label: "probe-inflated surface field",
            timestamps: None,
        });
        pass.set_pipeline(if gaussian {
            &self.generate_gaussian
        } else {
            &self.generate_union
        });
        pass.set_bind_group(1, output, &[]);
        pass.set_bind_group(2, representation, &[]);
        let [x, y, z] = dispatch_grid(dimensions);
        pass.dispatch(x, y, z);
    }

    pub(crate) fn record_erode(
        &self,
        encoder: &mut D::CommandEncoder,
        erosion: &D::BindGroup,
        representation: &D::BindGroup,
        dimensions: [u32; 3],
    ) {
        let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
            label: "rolling-probe surface erosion",
            timestamps: None,
        });
        pass.set_pipeline(&self.erode);
        pass.set_bind_group(1, erosion, &[]);
        pass.set_bind_group(2, representation, &[]);
        let [x, y, z] = dispatch_grid(dimensions);
        pass.dispatch(x, y, z);
    }
}

fn dispatch_grid(dimensions: [u32; 3]) -> [u32; 3] {
    dimensions.map(|axis| axis.div_ceil(WORKGROUP_EDGE))
}
