//! Pixel-stable atom points for dense semantic zoom.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::visual_pipelines::{VisualPipelineSet, constants};
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
    pipeline: VisualPipelineSet<D>,
    paged_pipeline: D::Pipeline,
    generic_pipeline: VisualPipelineSet<D>,
}

impl<D: Device> PointPass<D> {
    pub fn new(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
        paged_layout: &D::BindGroupLayout,
        generic_layout: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "geometry_point",
            wgsl: pdviewx_shaders::GEOMETRY_POINT,
        })?;
        let build = |pipeline_constants: &[(&'static str, f64)]| {
            device.create_render_pipeline(&RenderPipelineDesc {
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
                constants: pipeline_constants,
                topology: PrimitiveTopology::TriangleList,
            })
        };
        let paged_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "paged structure chunks",
            wgsl: pdviewx_shaders::PAGED_CHUNK,
        })?;
        let paged_pipeline = device.create_render_pipeline(&RenderPipelineDesc {
            label: "paged chunk points",
            layouts: &[Some(group0), Some(paged_layout)],
            shader: &paged_shader,
            vs_entry: "paged_point_vertex",
            fs_entry: Some("paged_point_fragment"),
            color_targets: &gbuffer_targets(),
            depth: Some(DepthState {
                format: TextureFormat::Depth32Float,
                write: true,
                compare: CompareFunction::GreaterEqual,
            }),
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        let generic_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "generic analytic points",
            wgsl: pdviewx_shaders::GENERIC_POINT,
        })?;
        let generic_build = |pipeline_constants: &[(&'static str, f64)]| {
            device.create_render_pipeline(&RenderPipelineDesc {
                label: "generic analytic points",
                layouts: &[Some(group0), None, Some(generic_layout)],
                shader: &generic_shader,
                vs_entry: "vs_generic_point",
                fs_entry: Some("fs_generic_point"),
                color_targets: &gbuffer_targets(),
                depth: Some(DepthState {
                    format: TextureFormat::Depth32Float,
                    write: true,
                    compare: CompareFunction::GreaterEqual,
                }),
                constants: pipeline_constants,
                topology: PrimitiveTopology::TriangleList,
            })
        };
        let generic_pipeline = VisualPipelineSet::new(
            generic_build(&[])?,
            generic_build(&constants(false))?,
            generic_build(&constants(true))?,
        );
        Ok(Self {
            pipeline: VisualPipelineSet::new(
                build(&[])?,
                build(&constants(false))?,
                build(&constants(true))?,
            ),
            paged_pipeline,
            generic_pipeline,
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
        if ctx.scene.point_draws(false).next().is_none()
            && ctx.scene.paged_point_draw().is_none()
            && ctx.scene.generic_point_draws(false).next().is_none()
        {
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
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        let mut bound = None;
        for (group, args, shading) in ctx.scene.point_draws(false) {
            if bound != Some(shading) {
                pass.set_pipeline(ctx.passes.point.pipeline.get(shading));
                bound = Some(shading);
            }
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(args, 0);
        }
        if let Some((group, args, offset)) = ctx.scene.paged_point_draw() {
            pass.set_pipeline(&ctx.passes.point.paged_pipeline);
            pass.set_bind_group(1, group, &[]);
            pass.draw_indirect(args, offset);
        }
        let mut generic_bound = None;
        for (group, args, shading) in ctx.scene.generic_point_draws(false) {
            if generic_bound != Some(shading) {
                pass.set_pipeline(ctx.passes.point.generic_pipeline.get(shading));
                generic_bound = Some(shading);
            }
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
