//! Opaque protein and nucleic-acid cartoon pass.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::visual_pipelines::{VisualPipelineSet, constants};
use crate::passes::{begin_gbuffer_pass, gbuffer_pipeline};
use crate::scene_gpu::DrawFamily;
use molgfx_gpu::{Device, RenderPassEncoder as _, ShaderModuleDesc};

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
        let pipeline = VisualPipelineSet::build(|set_constants| {
            gbuffer_pipeline(
                device,
                "cartoon ribbons",
                &[Some(group0), None, Some(group2)],
                &shader,
                ("vs_cartoon", "fs_cartoon"),
                set_constants,
            )
        })?;
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
        Ok(gbuffer_pipeline(
            device,
            "specialized cartoon ribbons",
            &[Some(group0), None, Some(ribbon)],
            &shader,
            ("vs_cartoon", "fs_cartoon"),
            &constants(true),
        )?)
    }

    pub(crate) fn record(ctx: &mut PassContext<'_, D>) {
        if ctx
            .scene
            .cartoon_draws(false, DrawFamily::Cartoon)
            .next()
            .is_none()
            && ctx.scene.mesh_draws(false).next().is_none()
        {
            return;
        }
        let passes = ctx.passes;
        let Some(cartoon) = ctx.build(&passes.cartoon, crate::passes::build::cartoon) else {
            return;
        };
        let Some(mut pass) = begin_gbuffer_pass(
            &mut *ctx.encoder,
            ctx.resources,
            ctx.timestamps,
            "cartoon ribbons",
        ) else {
            return;
        };
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        let mut bound: Option<*const D::Pipeline> = None;
        for draw in ctx
            .scene
            .cartoon_draws(false, DrawFamily::Cartoon)
            .chain(ctx.scene.mesh_draws(false))
        {
            let pipeline = cartoon.pipeline.select(draw.shading, draw.specialized);
            if bound != Some(std::ptr::from_ref(pipeline)) {
                pass.set_pipeline(pipeline);
                bound = Some(std::ptr::from_ref(pipeline));
            }
            pass.set_bind_group(2, draw.group, &[]);
            pass.set_index_buffer(draw.indices, molgfx_gpu::IndexFormat::Uint32);
            pass.draw_indexed_indirect(draw.arguments, 0);
        }
    }
}
