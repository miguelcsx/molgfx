//! Pixel-stable atom points for dense semantic zoom.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::visual_pipelines::{VisualPipelineSet, constants};
use crate::passes::{begin_gbuffer_pass, gbuffer_pipeline};
use molgfx_gpu::{Device, RenderPassEncoder as _, ShaderModuleDesc};

#[derive(Debug)]
pub(crate) struct PointPass<D: Device> {
    pipeline: VisualPipelineSet<D>,
    paged_pipeline: D::Pipeline,
    generic_pipeline: VisualPipelineSet<D>,
}

impl<D: Device> PointPass<D> {
    pub(crate) fn new(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
        paged_layout: &D::BindGroupLayout,
        generic_layout: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "geometry_point",
            wgsl: molgfx_shaders::GEOMETRY_POINT,
        })?;
        let pipeline = VisualPipelineSet::build(|set_constants| {
            gbuffer_pipeline(
                device,
                "atom points",
                &[Some(group0), None, Some(group2)],
                &shader,
                ("vs_point", "fs_point"),
                set_constants,
            )
        })?;
        let paged_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "paged structure chunks",
            wgsl: molgfx_shaders::PAGED_CHUNK,
        })?;
        let paged_pipeline = gbuffer_pipeline(
            device,
            "paged chunk points",
            &[Some(group0), Some(paged_layout)],
            &paged_shader,
            ("paged_point_vertex", "paged_point_fragment"),
            &[],
        )?;
        let generic_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "generic analytic points",
            wgsl: molgfx_shaders::GENERIC_POINT,
        })?;
        let generic_pipeline = VisualPipelineSet::build(|set_constants| {
            gbuffer_pipeline(
                device,
                "generic analytic points",
                &[Some(group0), None, Some(generic_layout)],
                &generic_shader,
                ("vs_generic_point", "fs_generic_point"),
                set_constants,
            )
        })?;
        Ok(Self {
            pipeline,
            paged_pipeline,
            generic_pipeline,
        })
    }

    /// Compiles the atom-point pipeline built from the generated sibling.
    ///
    /// # Errors
    ///
    /// Shader compilation or pipeline creation failed.
    pub(crate) fn build_specialized(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
    ) -> Result<D::Pipeline, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "point generated",
            wgsl: molgfx_shaders::GEOMETRY_POINT_SPECIALIZED,
        })?;
        Ok(gbuffer_pipeline(
            device,
            "specialized atom points",
            &[Some(group0), None, Some(group2)],
            &shader,
            ("vs_point", "fs_point"),
            &constants(true),
        )?)
    }

    pub(crate) fn record(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.point_draws(false).next().is_none()
            && ctx.scene.paged_point_draw().is_none()
            && ctx.scene.generic_point_draws(false).next().is_none()
        {
            return;
        }
        let passes = ctx.passes;
        let Some(point) = ctx.build(&passes.point, super::build::point) else {
            return;
        };
        let Some(mut pass) = begin_gbuffer_pass(
            &mut *ctx.encoder,
            ctx.resources,
            ctx.timestamps,
            "atom points",
        ) else {
            return;
        };
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        let mut bound: Option<*const D::Pipeline> = None;
        if let Some(arena) = ctx.scene.indirect_args() {
            for (group, offset, shading, specialized) in ctx.scene.point_draws(false) {
                let pipeline = point.pipeline.select(shading, specialized);
                if bound != Some(std::ptr::from_ref(pipeline)) {
                    pass.set_pipeline(pipeline);
                    bound = Some(std::ptr::from_ref(pipeline));
                }
                pass.set_bind_group(2, group, &[]);
                pass.draw_indirect(arena, offset);
            }
        }
        if let Some((group, args, offset)) = ctx.scene.paged_point_draw() {
            pass.set_pipeline(&point.paged_pipeline);
            pass.set_bind_group(1, group, &[]);
            pass.draw_indirect(args, offset);
        }
        let mut generic_bound = None;
        for (group, args, shading) in ctx.scene.generic_point_draws(false) {
            if generic_bound != Some(shading) {
                pass.set_pipeline(point.generic_pipeline.get(shading));
                generic_bound = Some(shading);
            }
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(args, 0);
        }
    }
}
