//! Pixel-stable atom points for dense semantic zoom.

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
pub struct PointPass<D: Device> {
    pipeline: D::Pipeline,
}

impl<D: Device> PointPass<D> {
    pub fn new(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "geometry_point",
            wgsl: pdviewx_shaders::GEOMETRY_POINT,
        })?;
        Ok(Self {
            pipeline: device.create_render_pipeline(&RenderPipelineDesc {
                label: "atom points",
                layouts: &[Some(group0), None, Some(group2)],
                shader: &shader,
                vs_entry: "vs_point",
                fs_entry: Some("fs_point"),
                color_targets: &[
                    gbuffer_targets()[0],
                    gbuffer_targets()[1],
                    gbuffer_targets()[2],
                    gbuffer_targets()[3],
                    gbuffer_targets()[4],
                ],
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
        if ctx.scene.point_draws(false).next().is_none() {
            return;
        }
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "atom points",
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
        pass.set_pipeline(&ctx.passes.point.pipeline);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        for (group, args, _) in ctx.scene.point_draws(false) {
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(args, 0);
        }
    }
}

const fn attachment<D: Device>(view: &D::TextureView) -> ColorAttachment<'_, D> {
    ColorAttachment {
        view,
        load: LoadOp::Load,
    }
}
