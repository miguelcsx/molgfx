//! GPU connected-component labeling for resident sampled-surface fields.

use crate::error::RenderError;
use molgfx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, CommandEncoder as _, ComputePassDesc,
    ComputePassEncoder as _, ComputePipelineDesc, Device, ShaderModuleDesc, ShaderStages,
    TextureFormat,
};

const WORKGROUP_EDGE: u32 = 4;

#[derive(Debug)]
pub(crate) struct SurfaceComponentPass<D: Device> {
    initialize: D::Pipeline,
    union: D::Pipeline,
    compress: D::Pipeline,
    measure: D::Pipeline,
    filter: D::Pipeline,
}

impl<D: Device> SurfaceComponentPass<D> {
    pub(crate) fn layout(device: &D) -> D::BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "surface component working set",
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
                        format: TextureFormat::R32Float,
                    },
                },
                storage(2),
                storage(3),
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Uniform,
                },
            ],
        })
    }

    pub(crate) fn new(device: &D, layout: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "surface connected components",
            wgsl: molgfx_shaders::SURFACE_COMPONENT_FILTER,
        })?;
        let pipeline = |label, entry| {
            device.create_compute_pipeline(&ComputePipelineDesc {
                label,
                layouts: &[None, Some(layout)],
                shader: &shader,
                entry,
            })
        };
        Ok(Self {
            initialize: pipeline("initialize surface components", "cs_component_initialize")?,
            union: pipeline("union surface components", "cs_component_union")?,
            compress: pipeline("compress surface components", "cs_component_compress")?,
            measure: pipeline("measure surface components", "cs_component_measure")?,
            filter: pipeline("filter surface components", "cs_component_filter")?,
        })
    }

    pub(crate) fn record(
        &self,
        encoder: &mut D::CommandEncoder,
        group: &D::BindGroup,
        dimensions: [u32; 3],
    ) {
        let dispatch = dimensions.map(|axis| axis.div_ceil(WORKGROUP_EDGE));
        for (label, pipeline) in [
            ("initialize surface components", &self.initialize),
            ("union surface components", &self.union),
        ] {
            let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
                label,
                timestamps: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(1, group, &[]);
            pass.dispatch(dispatch[0], dispatch[1], dispatch[2]);
        }
        let voxels = u64::from(dimensions[0]) * u64::from(dimensions[1]) * u64::from(dimensions[2]);
        let compression_passes = u64::BITS - voxels.max(1).leading_zeros();
        for _ in 0..compression_passes {
            let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
                label: "compress surface components",
                timestamps: None,
            });
            pass.set_pipeline(&self.compress);
            pass.set_bind_group(1, group, &[]);
            pass.dispatch(dispatch[0], dispatch[1], dispatch[2]);
        }
        for (label, pipeline) in [
            ("measure surface components", &self.measure),
            ("filter surface components", &self.filter),
        ] {
            let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
                label,
                timestamps: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(1, group, &[]);
            pass.dispatch(dispatch[0], dispatch[1], dispatch[2]);
        }
    }
}

const fn storage(binding: u32) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Storage { read_only: false },
    }
}
