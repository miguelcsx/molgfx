//! Deterministic compute decluttering and one-draw analytic label rendering.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::{
    DEPTH_RESOURCE, ENTITY_RESOURCE, OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE, STRUCTURE_RESOURCE,
};
use pdviewx_gpu::{
    BlendMode, ColorAttachment, ColorTarget, CommandEncoder as _, CompareFunction, ComputePassDesc,
    ComputePassEncoder as _, ComputePipelineDesc, DepthAttachment, DepthLoadOp, DepthState, Device,
    LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _, RenderPipelineDesc,
    ShaderModuleDesc, TextureFormat,
};

#[derive(Debug)]
pub struct LabelPass<D: Device> {
    declutter: D::Pipeline,
    render: D::Pipeline,
}

impl<D: Device> LabelPass<D> {
    pub fn new(
        device: &D,
        frame: &D::BindGroupLayout,
        declutter_layout: &D::BindGroupLayout,
        render_layout: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let declutter_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "label decluttering",
            wgsl: pdviewx_shaders::LABEL_DECLUTTER,
        })?;
        let render_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "analytic semantic labels",
            wgsl: pdviewx_shaders::GEOMETRY_LABEL,
        })?;
        Ok(Self {
            declutter: device.create_compute_pipeline(&ComputePipelineDesc {
                label: "deterministic label decluttering",
                layouts: &[Some(frame), None, Some(declutter_layout)],
                shader: &declutter_shader,
                entry: "declutter_labels",
            })?,
            render: device.create_render_pipeline(&RenderPipelineDesc {
                label: "analytic semantic labels",
                layouts: &[Some(frame), None, Some(render_layout)],
                shader: &render_shader,
                vs_entry: "vs_label",
                fs_entry: Some("fs_label"),
                color_targets: &label_targets(),
                depth: Some(DepthState {
                    format: TextureFormat::Depth32Float,
                    write: false,
                    compare: CompareFunction::GreaterEqual,
                }),
                constants: &[],
                topology: PrimitiveTopology::TriangleList,
            })?,
        })
    }

    pub fn declutter(ctx: &mut PassContext<'_, D>) {
        let Some(group) = ctx.scene.label_declutter() else {
            return;
        };
        let mut pass = ctx.encoder.begin_compute_pass(&ComputePassDesc {
            label: "deterministic label decluttering",
            timestamps: ctx.timestamps,
        });
        pass.set_pipeline(&ctx.passes.label.declutter);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(2, group, &[]);
        pass.dispatch(1, 1, 1);
    }

    pub fn render(ctx: &mut PassContext<'_, D>) {
        let Some((group, args)) = ctx.scene.label_draw() else {
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
            label: "analytic semantic labels",
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
        pass.set_pipeline(&ctx.passes.label.render);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(2, group, &[]);
        pass.draw_indirect(args, 0);
    }
}

const fn label_targets() -> [ColorTarget; 4] {
    [
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
    ]
}
