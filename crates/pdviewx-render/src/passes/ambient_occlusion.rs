//! Deterministic screen-space ambient occlusion for molecular cavities.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::{AO_RESOURCE, FrameBindings};
use pdviewx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, ColorAttachment, ColorTarget,
    CommandEncoder as _, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, ShaderStages, TextureFormat,
};

#[derive(Debug)]
pub struct AmbientOcclusionPass<D: Device> {
    realtime: D::Pipeline,
    quality: D::Pipeline,
    pub(crate) layout: D::BindGroupLayout,
}

impl<D: Device> AmbientOcclusionPass<D> {
    pub fn new(
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
            wgsl: pdviewx_shaders::AMBIENT_OCCLUSION,
        })?;
        let realtime = device.create_render_pipeline(&RenderPipelineDesc {
            label: "molecular ambient occlusion",
            layouts: &[Some(group0), Some(&layout)],
            shader: &shader,
            vs_entry: "vs_fullscreen",
            fs_entry: Some("fs_ambient_occlusion"),
            color_targets: &[ColorTarget {
                format: TextureFormat::Rgba8Unorm,
                blend: pdviewx_gpu::BlendMode::Replace,
            }],
            depth: None,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        let quality_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "quality_ao",
            wgsl: pdviewx_shaders::QUALITY_AO,
        })?;
        let quality = device.create_render_pipeline(&RenderPipelineDesc {
            label: "progressive molecular ray-traced occlusion",
            layouts: &[Some(group0), Some(&layout), Some(group2)],
            shader: &quality_shader,
            vs_entry: "vs_fullscreen",
            fs_entry: Some("fs_quality_ao"),
            color_targets: &[ColorTarget {
                format: TextureFormat::Rgba8Unorm,
                blend: pdviewx_gpu::BlendMode::ReverseMultiply,
            }],
            depth: None,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        Ok(Self {
            realtime,
            quality,
            layout,
        })
    }

    pub fn record(ctx: &mut PassContext<'_, D>) {
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
            pass.set_pipeline(&ctx.passes.ambient_occlusion.quality);
            pass.set_bind_group(0, &ctx.scene.group0, &[]);
            pass.set_bind_group(1, ao, &[]);
            for group in ctx.scene.quality_draws() {
                pass.set_bind_group(2, group, &[]);
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
