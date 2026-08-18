//! Depth-independent screen overlays composed after tonemapping.

use crate::error::RenderError;
use crate::graph::{PassContext, ResourceId};
use pdviewx_gpu::{
    BlendMode, ColorAttachment, ColorTarget, CommandEncoder as _, Device, LoadOp,
    PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _, RenderPipelineDesc,
    ShaderModuleDesc, TextureFormat,
};

#[derive(Debug)]
pub struct OverlayPass<D: Device> {
    pipeline: D::Pipeline,
}

impl<D: Device> OverlayPass<D> {
    pub fn new(
        device: &D,
        target_format: TextureFormat,
        frame: &D::BindGroupLayout,
        overlays: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "screen overlays",
            wgsl: pdviewx_shaders::OVERLAY,
        })?;
        Ok(Self {
            pipeline: device.create_render_pipeline(&RenderPipelineDesc {
                label: "screen overlays",
                layouts: &[Some(frame), None, Some(overlays)],
                shader: &shader,
                vs_entry: "vs_overlay",
                fs_entry: Some("fs_overlay"),
                color_targets: &[ColorTarget {
                    format: target_format,
                    blend: BlendMode::Alpha,
                }],
                depth: None,
                constants: &[],
                topology: PrimitiveTopology::TriangleList,
            })?,
        })
    }

    pub fn record(ctx: &mut PassContext<'_, D>) {
        let (Some(target), Some((group, args))) = (
            ctx.resources.view(ResourceId::SWAPCHAIN),
            ctx.scene.overlay_draw(),
        ) else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "screen overlays",
            colors: &[ColorAttachment {
                view: target,
                load: LoadOp::Load,
            }],
            depth: None,
            timestamps: ctx.timestamps,
        });
        pass.set_pipeline(&ctx.passes.overlay.pipeline);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(2, group, &[]);
        pass.draw_indirect(args, 0);
    }
}
