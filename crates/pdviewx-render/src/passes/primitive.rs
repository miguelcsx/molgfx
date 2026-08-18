//! Analytic caller-authored primitive pass.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::{
    ALBEDO_RESOURCE, DEPTH_RESOURCE, ENTITY_RESOURCE, MOTION_RESOURCE, NORMAL_RESOURCE,
    STRUCTURE_RESOURCE, gbuffer_targets,
};
use pdviewx_gpu::{
    ColorAttachment, CommandEncoder as _, CompareFunction, DepthAttachment, DepthLoadOp,
    DepthState, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, TextureFormat,
};

#[derive(Debug)]
pub struct PrimitivePass<D: Device> {
    pub(crate) pipeline: D::Pipeline,
}

impl<D: Device> PrimitivePass<D> {
    pub fn new(
        device: &D,
        group0: &D::BindGroupLayout,
        primitive: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "geometry_primitive",
            wgsl: pdviewx_shaders::GEOMETRY_PRIMITIVE,
        })?;
        Ok(Self {
            pipeline: device.create_render_pipeline(&RenderPipelineDesc {
                label: "analytic primitives",
                layouts: &[Some(group0), None, Some(primitive)],
                shader: &shader,
                vs_entry: "vs_primitive",
                fs_entry: Some("fs_primitive"),
                color_targets: &gbuffer_targets(),
                depth: Some(DepthState {
                    format: TextureFormat::Depth32Float,
                    write: true,
                    compare: CompareFunction::GreaterEqual,
                }),
                constants: &[],
                topology: PrimitiveTopology::TriangleList,
            })?,
        })
    }

    pub fn record(ctx: &mut PassContext<'_, D>) {
        let (Some(albedo), Some(normal), Some(entity), Some(structure), Some(motion), Some(depth)) = (
            ctx.resources.view(ALBEDO_RESOURCE),
            ctx.resources.view(NORMAL_RESOURCE),
            ctx.resources.view(ENTITY_RESOURCE),
            ctx.resources.view(STRUCTURE_RESOURCE),
            ctx.resources.view(MOTION_RESOURCE),
            ctx.resources.view(DEPTH_RESOURCE),
        ) else {
            return;
        };
        let Some((group, args)) = ctx.scene.primitive_draw() else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "analytic primitives",
            colors: &[
                attachment(albedo),
                attachment(normal),
                attachment(entity),
                attachment(structure),
                attachment(motion),
            ],
            depth: Some(DepthAttachment {
                view: depth,
                load: DepthLoadOp::Load,
                read_only: false,
            }),
            timestamps: ctx.timestamps,
        });
        pass.set_pipeline(&ctx.passes.primitive.pipeline);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(2, group, &[]);
        pass.draw_indirect(args, 0);
    }
}

const fn attachment<D: Device>(view: &D::TextureView) -> ColorAttachment<'_, D> {
    ColorAttachment {
        view,
        load: LoadOp::Load,
    }
}
