//! HDR deferred lighting over the shared molecular gbuffer.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::{FrameBindings, HDR_RESOURCE};
use pdviewx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, ColorAttachment, ColorTarget,
    CommandEncoder as _, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, ShaderStages, TextureFormat,
};

#[derive(Debug)]
pub struct LightingPass<D: Device> {
    pipeline: D::Pipeline,
    pub(crate) layout: D::BindGroupLayout,
}

impl<D: Device> LightingPass<D> {
    pub fn new(device: &D, group0: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let texture = |binding| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture { filterable: true },
        };
        let layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group1: deferred lighting inputs",
            entries: &[
                texture(0),
                texture(1),
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::DepthTexture,
                },
                texture(3),
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::DepthTexture,
                },
            ],
        });
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "lighting",
            wgsl: pdviewx_shaders::LIGHTING,
        })?;
        let pipeline = device.create_render_pipeline(&RenderPipelineDesc {
            label: "HDR deferred lighting",
            layouts: &[Some(group0), Some(&layout)],
            shader: &shader,
            vs_entry: "vs_fullscreen",
            fs_entry: Some("fs_lighting"),
            color_targets: &[ColorTarget {
                format: TextureFormat::Rgba16Float,
                blend: pdviewx_gpu::BlendMode::Replace,
            }],
            depth: None,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        Ok(Self { pipeline, layout })
    }

    pub fn record(ctx: &mut PassContext<'_, D>) {
        let Some(target) = ctx.resources.view(HDR_RESOURCE) else {
            return;
        };
        let Some(FrameBindings { lighting, .. }) = ctx.bindings else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "HDR deferred lighting",
            colors: &[ColorAttachment {
                view: target,
                load: LoadOp::Clear([0.0, 0.0, 0.0, 1.0]),
            }],
            depth: None,
            timestamps: ctx.timestamps,
        });
        pass.set_pipeline(&ctx.passes.lighting.pipeline);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(1, lighting, &[]);
        pass.draw(0..3, 0..1);
    }
}
