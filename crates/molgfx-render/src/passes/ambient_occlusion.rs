//! Deterministic screen-space ambient occlusion for molecular cavities.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::visual_pipelines::{VisualPipelineSet, constants};
use crate::passes::{AO_RESOURCE, FrameBindings};
use molgfx_gpu::{
    AccelerationStructureLayoutEntry, BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType,
    ColorAttachment, ColorTarget, CommandEncoder as _, Device, LoadOp, PrimitiveTopology,
    RayQueryBindGroupLayoutDesc, RenderPassDesc, RenderPassEncoder as _, RenderPipelineDesc,
    ShaderModuleDesc, ShaderStages, TextureFormat,
};

#[cfg(test)]
#[path = "ambient_occlusion_tests.rs"]
mod tests;

#[derive(Debug)]
pub(crate) struct AmbientOcclusionPass<D: Device> {
    realtime: D::Pipeline,
    quality: VisualPipelineSet<D>,
    hardware: Option<HardwareQualityPipelines<D>>,
    pub(crate) layout: D::BindGroupLayout,
}

#[derive(Debug)]
struct HardwareQualityPipelines<D: Device> {
    pipelines: VisualPipelineSet<D>,
    layout: D::BindGroupLayout,
}

impl<D: Device> AmbientOcclusionPass<D> {
    pub(crate) fn new(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group1: ambient occlusion inputs",
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::DepthTexture,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture { filterable: true },
                },
            ],
        });
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "ambient_occlusion",
            wgsl: molgfx_shaders::AMBIENT_OCCLUSION,
        })?;
        let realtime = device.create_render_pipeline(&RenderPipelineDesc {
            label: "molecular ambient occlusion",
            layouts: &[Some(group0), Some(&layout)],
            shader: &shader,
            vs_entry: "vs_fullscreen",
            fs_entry: Some("fs_ambient_occlusion"),
            color_targets: &[ColorTarget {
                format: TextureFormat::Rgba8Unorm,
                blend: molgfx_gpu::BlendMode::Replace,
            }],
            depth: None,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        let quality_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "quality_ao",
            wgsl: molgfx_shaders::QUALITY_AO,
        })?;
        let quality_pipeline = |pipeline_constants: &[(&'static str, f64)]| {
            device.create_render_pipeline(&RenderPipelineDesc {
                label: "progressive molecular ray-traced occlusion",
                layouts: &[Some(group0), Some(&layout), Some(group2)],
                shader: &quality_shader,
                vs_entry: "vs_fullscreen",
                fs_entry: Some("fs_quality_ao"),
                color_targets: &[ColorTarget {
                    format: TextureFormat::Rgba8Unorm,
                    blend: molgfx_gpu::BlendMode::ReverseMultiply,
                }],
                depth: None,
                constants: pipeline_constants,
                topology: PrimitiveTopology::TriangleList,
            })
        };
        let quality = VisualPipelineSet::new(
            quality_pipeline(&[])?,
            quality_pipeline(&constants(false))?,
            quality_pipeline(&constants(true))?,
        );
        let hardware = create_hardware_quality(device, group0, &layout, group2);
        Ok(Self {
            realtime,
            quality,
            hardware,
            layout,
        })
    }

    pub(crate) fn ray_query_layout(&self) -> Option<&D::BindGroupLayout> {
        self.hardware.as_ref().map(|hardware| &hardware.layout)
    }

    pub(crate) fn record(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.is_massive_points_only() {
            return;
        }
        let Some(target) = ctx.resources.view(AO_RESOURCE) else {
            return;
        };
        let Some(FrameBindings { ao, .. }) = ctx.bindings else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "molecular ambient occlusion",
            colors: &[ColorAttachment {
                view: target,
                load: LoadOp::Clear([1.0, 1.0, 1.0, 1.0]),
            }],
            depth: None,
            timestamps: ctx.timestamps,
        });
        if ctx.quality {
            pass.set_bind_group(0, &ctx.scene.group0, &[]);
            pass.set_bind_group(1, ao, &[]);
            let mut bound = None;
            for (group, ray_group, shading) in ctx.scene.quality_draws() {
                let hardware = ctx
                    .passes
                    .ambient_occlusion
                    .hardware
                    .as_ref()
                    .zip(ray_group);
                let pipeline_key = (shading, hardware.is_some());
                if bound != Some(pipeline_key) {
                    let pipeline = match hardware {
                        Some((hardware, _)) => hardware.pipelines.get(shading),
                        None => ctx.passes.ambient_occlusion.quality.get(shading),
                    };
                    pass.set_pipeline(pipeline);
                    bound = Some(pipeline_key);
                }
                pass.set_bind_group(2, group, &[]);
                if let Some((_, ray_group)) = hardware {
                    pass.set_bind_group(3, ray_group, &[]);
                }
                pass.draw(0..3, 0..1);
            }
            return;
        }
        pass.set_pipeline(&ctx.passes.ambient_occlusion.realtime);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(1, ao, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn create_hardware_quality<D: Device>(
    device: &D,
    group0: &D::BindGroupLayout,
    group1: &D::BindGroupLayout,
    group2: &D::BindGroupLayout,
) -> Option<HardwareQualityPipelines<D>> {
    if !device.capabilities().ray_query() {
        return None;
    }
    let Ok(layout) = device.create_ray_query_bind_group_layout(&RayQueryBindGroupLayoutDesc {
        label: "group3: quality ray-query scene",
        entries: &[],
        acceleration_structures: &[AccelerationStructureLayoutEntry {
            binding: 0,
            visibility: ShaderStages::FRAGMENT,
        }],
    }) else {
        return None;
    };
    let Ok(shader) = device.create_shader_module(&ShaderModuleDesc {
        label: "quality_ao_ray_query",
        wgsl: molgfx_shaders::QUALITY_AO_RAY_QUERY,
    }) else {
        return None;
    };
    let pipeline = |pipeline_constants: &[(&'static str, f64)]| {
        device.create_render_pipeline(&RenderPipelineDesc {
            label: "hardware progressive molecular occlusion",
            layouts: &[Some(group0), Some(group1), Some(group2), Some(&layout)],
            shader: &shader,
            vs_entry: "vs_fullscreen",
            fs_entry: Some("fs_quality_ao"),
            color_targets: &[ColorTarget {
                format: TextureFormat::Rgba8Unorm,
                blend: molgfx_gpu::BlendMode::ReverseMultiply,
            }],
            depth: None,
            constants: pipeline_constants,
            topology: PrimitiveTopology::TriangleList,
        })
    };
    let Ok(base) = pipeline(&[]) else {
        return None;
    };
    let Ok(wire) = pipeline(&constants(false)) else {
        return None;
    };
    let Ok(clipped) = pipeline(&constants(true)) else {
        return None;
    };
    Some(HardwareQualityPipelines {
        pipelines: VisualPipelineSet::new(base, wire, clipped),
        layout,
    })
}
