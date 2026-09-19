//! Depth-independent screen overlays composed after tonemapping.

use crate::error::RenderError;
use crate::graph::{PassContext, ResourceId};
use molgfx_gpu::{
    BlendMode, ColorAttachment, ColorTarget, CommandEncoder as _, Device, LoadOp,
    PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _, RenderPipelineDesc,
    ShaderModuleDesc, TextureFormat,
};

#[derive(Debug)]
pub(crate) struct OverlayPass<D: Device> {
    /// One pipeline per overlay kind — glyph, gradient, scale, axis. Each
    /// shares the overlay instance array and skips the kinds it does not draw.
    pipelines: [D::Pipeline; 4],
}

impl<D: Device> OverlayPass<D> {
    pub(crate) fn new(
        device: &D,
        target_format: TextureFormat,
        frame: &D::BindGroupLayout,
        overlays: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "screen overlays",
            wgsl: molgfx_shaders::OVERLAY,
        })?;
        let targets = [ColorTarget {
            format: target_format,
            blend: BlendMode::Alpha,
        }];
        let build = |label, vs_entry, fs_entry| {
            device.create_render_pipeline(&RenderPipelineDesc {
                label,
                layouts: &[Some(frame), None, Some(overlays)],
                shader: &shader,
                vs_entry,
                fs_entry: Some(fs_entry),
                color_targets: &targets,
                depth: None,
                constants: &[],
                topology: PrimitiveTopology::TriangleList,
            })
        };
        Ok(Self {
            pipelines: [
                build("overlay glyphs", "vs_overlay_glyph", "fs_overlay_glyph")?,
                build(
                    "overlay gradients",
                    "vs_overlay_gradient",
                    "fs_overlay_gradient",
                )?,
                build("overlay scale bars", "vs_overlay_scale", "fs_overlay_line")?,
                build("overlay axes", "vs_overlay_axis", "fs_overlay_line")?,
            ],
        })
    }

    pub(crate) fn record(ctx: &mut PassContext<'_, D>) {
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
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(2, group, &[]);
        for pipeline in &ctx.passes.overlay.pipelines {
            pass.set_pipeline(pipeline);
            pass.draw_indirect(args, 0);
        }
    }
}
