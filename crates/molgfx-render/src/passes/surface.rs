//! BVH-bounded implicit molecular surface pass.
//!
//! One indirect fullscreen draw is recorded per surface representation. GPU
//! work scales with covered pixels, marching steps and visited hierarchy nodes;
//! no atom loop or surface mesh exists on the CPU frame path.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::visual_pipelines::{VisualPipelineSet, constants};
use crate::passes::{begin_gbuffer_pass, gbuffer_targets};
use molgfx_gpu::{
    CompareFunction, DepthState, Device, PrimitiveTopology, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, TextureFormat,
};

#[derive(Debug)]
pub(crate) struct SurfacePass<D: Device> {
    /// Analytic union of atom spheres: van der Waals and solvent-accessible.
    union_surface: VisualPipelineSet<D>,
    /// Ray march through the persistent field: solvent-excluded and Gaussian.
    grid_surface: VisualPipelineSet<D>,
}

impl<D: Device> SurfacePass<D> {
    pub(crate) fn new(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "implicit molecular surfaces",
            wgsl: molgfx_shaders::GEOMETRY_SURFACE,
        })?;
        // The two tracings share a vertex stage but nothing else, so each is
        // its own pipeline: an analytic surface never carries the grid march,
        // and a field surface never carries the hierarchy walk.
        let pipeline = |label, fs_entry| -> Result<VisualPipelineSet<D>, RenderError> {
            let build = |pipeline_constants: &[(&'static str, f64)]| {
                device.create_render_pipeline(&RenderPipelineDesc {
                    label,
                    layouts: &[Some(group0), None, Some(group2)],
                    shader: &shader,
                    vs_entry: "vs_surface",
                    fs_entry: Some(fs_entry),
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
            Ok(VisualPipelineSet::new(
                build(&[])?,
                build(&constants(false))?,
                build(&constants(true))?,
            ))
        };
        Ok(Self {
            union_surface: pipeline("analytic molecular surfaces", "fs_surface_union")?,
            grid_surface: pipeline("field molecular surfaces", "fs_surface_grid")?,
        })
    }

    /// Compiles a surface tracing built from the generated sibling.
    ///
    /// # Errors
    ///
    /// Shader compilation or pipeline creation failed.
    pub(crate) fn build_specialized(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
        grid: bool,
    ) -> Result<D::Pipeline, RenderError> {
        let (label, fs_entry) = if grid {
            ("specialized field molecular surfaces", "fs_surface_grid")
        } else {
            (
                "specialized analytic molecular surfaces",
                "fs_surface_union",
            )
        };
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "surface generated",
            wgsl: molgfx_shaders::GEOMETRY_SURFACE_SPECIALIZED,
        })?;
        Ok(device.create_render_pipeline(&RenderPipelineDesc {
            label,
            layouts: &[Some(group0), None, Some(group2)],
            shader: &shader,
            vs_entry: "vs_surface",
            fs_entry: Some(fs_entry),
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
        if ctx.scene.surface_draws(false).next().is_none() {
            return;
        }
        let passes = ctx.passes;
        let Some(surface) = ctx.build(&passes.surface, super::build::surface) else {
            return;
        };
        let Some(mut pass) = begin_gbuffer_pass(
            &mut *ctx.encoder,
            ctx.resources,
            ctx.timestamps,
            "implicit molecular surfaces",
        ) else {
            return;
        };
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        let mut bound: Option<*const D::Pipeline> = None;
        if let Some(arena) = ctx.scene.indirect_args() {
            for (group, offset, shading, specialized) in ctx.scene.surface_draws(false) {
                let pipeline = if shading.surface_grid() {
                    surface.grid_surface.select(shading, specialized)
                } else {
                    surface.union_surface.select(shading, specialized)
                };
                if bound != Some(std::ptr::from_ref(pipeline)) {
                    pass.set_pipeline(pipeline);
                    bound = Some(std::ptr::from_ref(pipeline));
                }
                pass.set_bind_group(2, group, &[]);
                pass.draw_indirect(arena, offset);
            }
        }
    }
}
