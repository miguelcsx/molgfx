//! Full-screen weighted transparency composition into temporal HDR input.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::{COMPOSITE_RESOURCE, FrameBindings};
use molgfx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, BlendMode, ColorAttachment,
    ColorTarget, CommandEncoder as _, Device, LoadOp, PrimitiveTopology, RenderPassDesc,
    RenderPassEncoder as _, RenderPipelineDesc, ShaderModuleDesc, ShaderStages, TextureFormat,
};

#[derive(Debug)]
pub(crate) struct OitCompositePass<D: Device> {
    pipeline: D::Pipeline,
}

impl<D: Device> OitCompositePass<D> {
    /// The inputs the composite reads; the frame's bind groups are made over
    /// it whether or not the pipeline exists yet.
    pub(crate) fn layout(device: &D) -> D::BindGroupLayout {
        let texture = |binding| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture { filterable: true },
        };
        device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group1: transparency composite inputs",
            entries: &[texture(0), texture(1), texture(2)],
        })
    }

    pub(crate) fn new(device: &D, layout: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "weighted transparency composite",
            wgsl: molgfx_shaders::OIT_COMPOSITE,
        })?;
        let pipeline = device.create_render_pipeline(&RenderPipelineDesc {
            label: "weighted transparency composite",
            layouts: &[None, Some(layout)],
            shader: &shader,
            vs_entry: "vs_fullscreen",
            fs_entry: Some("fs_oit_composite"),
            color_targets: &[ColorTarget {
                format: TextureFormat::Rgba16Float,
                blend: BlendMode::Replace,
            }],
            depth: None,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        Ok(Self { pipeline })
    }

    pub(crate) fn record(ctx: &mut PassContext<'_, D>) {
        if !ctx.scene.has_translucency() {
            return;
        }
        let Some(target) = ctx.resources.view(COMPOSITE_RESOURCE) else {
            return;
        };
        let Some(FrameBindings { oit_composite, .. }) = ctx.bindings else {
            return;
        };
        let passes = ctx.passes;
        let Some(composite) = ctx.build(&passes.oit_composite, |env| {
            OitCompositePass::new(env.device, &passes.oit_composite_layout)
        }) else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "weighted transparency composite",
            colors: &[ColorAttachment {
                view: target,
                load: LoadOp::Clear([0.0, 0.0, 0.0, 1.0]),
            }],
            depth: None,
            timestamps: ctx.timestamps,
        });
        pass.set_pipeline(&composite.pipeline);
        pass.set_bind_group(1, oit_composite, &[]);
        pass.draw(0..3, 0..1);
    }
}
