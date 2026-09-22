//! Opaque protein and nucleic-acid cartoon pass.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::visual_pipelines::{VisualPipelineSet, constants};
use crate::passes::{
    ALBEDO_RESOURCE, DEPTH_RESOURCE, ENTITY_RESOURCE, MOTION_RESOURCE, NORMAL_RESOURCE,
    STRUCTURE_RESOURCE, gbuffer_targets,
};
use crate::scene_gpu::DrawFamily;
use molgfx_gpu::{
    ColorAttachment, CommandEncoder as _, CompareFunction, DepthAttachment, DepthLoadOp,
    DepthState, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, TextureFormat,
};

#[derive(Debug)]
pub(crate) struct CartoonPass<D: Device> {
    pipeline: VisualPipelineSet<D>,
}

impl<D: Device> CartoonPass<D> {
    pub(crate) fn new(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "geometry_cartoon",
            wgsl: molgfx_shaders::GEOMETRY_CARTOON,
        })?;
        let targets = gbuffer_targets();
        let build = |pipeline_constants: &[(&'static str, f64)]| {
            device.create_render_pipeline(&RenderPipelineDesc {
                label: "cartoon ribbons",
                layouts: &[Some(group0), None, Some(group2)],
                shader: &shader,
                vs_entry: "vs_cartoon",
                fs_entry: Some("fs_cartoon"),
                color_targets: &targets,
                depth: Some(DepthState {
                    format: TextureFormat::Depth32Float,
                    write: true,
                    compare: CompareFunction::GreaterEqual,
                }),
                constants: pipeline_constants,
                topology: PrimitiveTopology::TriangleList,
            })
        };
        let pipeline = VisualPipelineSet::new(
            build(&[])?,
            build(&constants(false))?,
            build(&constants(true))?,
        );
        Ok(Self { pipeline })
    }

    /// Compiles the cartoon ribbon pipeline built from the generated sibling.
    ///
    /// # Errors
    ///
    /// Shader compilation or pipeline creation failed.
    pub(crate) fn build_specialized(
        device: &D,
        group0: &D::BindGroupLayout,
        ribbon: &D::BindGroupLayout,
    ) -> Result<D::Pipeline, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "cartoon generated",
            wgsl: molgfx_shaders::GEOMETRY_CARTOON_SPECIALIZED,
        })?;
        Ok(device.create_render_pipeline(&RenderPipelineDesc {
            label: "specialized cartoon ribbons",
            layouts: &[Some(group0), None, Some(ribbon)],
            shader: &shader,
            vs_entry: "vs_cartoon",
            fs_entry: Some("fs_cartoon"),
            color_targets: &gbuffer_targets(),
            depth: Some(DepthState {
                format: TextureFormat::Depth32Float,
                write: true,
                compare: CompareFunction::GreaterEqual,
            }),
            constants: &constants(true),
            topology: PrimitiveTopology::TriangleList,
        })?)
    }

    pub(crate) fn record(ctx: &mut PassContext<'_, D>) {
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
        if ctx
            .scene
            .cartoon_draws(false, DrawFamily::Cartoon)
            .next()
            .is_none()
            && ctx.scene.mesh_draws(false).next().is_none()
        {
            return;
        }
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "cartoon ribbons",
            colors: &[
                ColorAttachment {
                    view: albedo,
                    load: LoadOp::Load,
                },
                ColorAttachment {
                    view: normal,
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
                ColorAttachment {
                    view: motion,
                    load: LoadOp::Load,
                },
            ],
            depth: Some(DepthAttachment {
                view: depth,
                load: DepthLoadOp::Load,
                read_only: false,
            }),
            timestamps: ctx.timestamps,
        });
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        let mut bound: Option<*const D::Pipeline> = None;
        for (group2, args, shading, specialized) in ctx
            .scene
            .cartoon_draws(false, DrawFamily::Cartoon)
            .chain(ctx.scene.mesh_draws(false))
        {
            let pipeline = ctx.passes.cartoon.pipeline.select(shading, specialized);
            if bound != Some(std::ptr::from_ref(pipeline)) {
                pass.set_pipeline(pipeline);
                bound = Some(std::ptr::from_ref(pipeline));
            }
            pass.set_bind_group(2, group2, &[]);
            pass.draw_indirect(args, 0);
        }
    }
}
