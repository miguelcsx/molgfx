//! Single-draw weighted-transparency pass for caller-supplied interactions.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::{
    DEPTH_RESOURCE, ENTITY_RESOURCE, OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE, STRUCTURE_RESOURCE,
};
use molgfx_gpu::{
    BlendMode, ColorAttachment, ColorTarget, CommandEncoder as _, CompareFunction, DepthAttachment,
    DepthLoadOp, DepthState, Device, LoadOp, PrimitiveTopology, RenderPassDesc,
    RenderPassEncoder as _, RenderPipelineDesc, ShaderModuleDesc, TextureFormat,
};

#[derive(Debug)]
pub struct InteractionPass<D: Device> {
    pipeline: D::Pipeline,
}

impl<D: Device> InteractionPass<D> {
    pub fn new(
        device: &D,
        frame: &D::BindGroupLayout,
        interactions: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "interaction glyphs",
            wgsl: molgfx_shaders::GEOMETRY_INTERACTION,
        })?;
        let pipeline = device.create_render_pipeline(&RenderPipelineDesc {
            label: "interaction glyphs",
            layouts: &[Some(frame), None, Some(interactions)],
            shader: &shader,
            vs_entry: "vs_interaction",
            fs_entry: Some("fs_interaction"),
            color_targets: &[
                ColorTarget {
                    format: TextureFormat::Rgba16Float,
                    blend: BlendMode::Additive,
                },
                ColorTarget {
                    format: TextureFormat::R8Unorm,
                    blend: BlendMode::ReverseMultiply,
                },
                ColorTarget {
                    format: TextureFormat::R32Uint,
                    blend: BlendMode::Replace,
                },
                ColorTarget {
                    format: TextureFormat::R32Uint,
                    blend: BlendMode::Replace,
                },
            ],
            depth: Some(DepthState {
                format: TextureFormat::Depth32Float,
                write: false,
                compare: CompareFunction::GreaterEqual,
            }),
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        Ok(Self { pipeline })
    }

    pub fn record(ctx: &mut PassContext<'_, D>) {
        let Some((interaction_group, args)) = ctx.scene.interaction_draw() else {
            return;
        };
        let (Some(accumulation), Some(revealage), Some(entity), Some(structure), Some(depth)) = (
            ctx.resources.view(OIT_ACCUM_RESOURCE),
            ctx.resources.view(OIT_REVEAL_RESOURCE),
            ctx.resources.view(ENTITY_RESOURCE),
            ctx.resources.view(STRUCTURE_RESOURCE),
            ctx.resources.view(DEPTH_RESOURCE),
        ) else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "interaction glyphs",
            colors: &[
                ColorAttachment {
                    view: accumulation,
                    load: LoadOp::Load,
                },
                ColorAttachment {
                    view: revealage,
                    load: LoadOp::Load,
                },
                ColorAttachment {
                    view: entity,
                    load: LoadOp::Load,
                },
                ColorAttachment {
                    view: structure,
                    load: LoadOp::Load,
                },
            ],
            depth: Some(DepthAttachment {
                view: depth,
                load: DepthLoadOp::Load,
                read_only: true,
            }),
            timestamps: ctx.timestamps,
        });
        pass.set_pipeline(&ctx.passes.interaction.pipeline);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(2, interaction_group, &[]);
        pass.draw_indirect(args, 0);
    }
}
