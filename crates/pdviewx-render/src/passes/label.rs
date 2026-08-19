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
    /// One pipeline per label kind — glyph, guide, marker. Each shares the
    /// decluttered instance array and skips the kinds it does not draw, so the
    /// three specialized fragment routines never branch per fragment.
    render: [D::Pipeline; 3],
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
            render: label_pipelines(device, frame, render_layout, &render_shader)?,
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
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(2, group, &[]);
        for pipeline in &ctx.passes.label.render {
            pass.set_pipeline(pipeline);
            pass.draw_indirect(args, 0);
        }
    }
}

fn label_pipelines<D: Device>(
    device: &D,
    frame: &D::BindGroupLayout,
    render_layout: &D::BindGroupLayout,
    shader: &D::ShaderModule,
) -> Result<[D::Pipeline; 3], RenderError> {
    let build = |label, vs_entry, fs_entry| {
        device.create_render_pipeline(&RenderPipelineDesc {
            label,
            layouts: &[Some(frame), None, Some(render_layout)],
            shader,
            vs_entry,
            fs_entry: Some(fs_entry),
            color_targets: &label_targets(),
            depth: Some(DepthState {
                format: TextureFormat::Depth32Float,
                write: false,
                compare: CompareFunction::GreaterEqual,
            }),
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })
    };
    Ok([
        build("semantic label glyphs", "vs_label_glyph", "fs_label_glyph")?,
        build("semantic label guides", "vs_label_guide", "fs_label_guide")?,
        build(
            "semantic label markers",
            "vs_label_marker",
            "fs_label_marker",
        )?,
    ])
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
