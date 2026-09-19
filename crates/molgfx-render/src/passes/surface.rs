//! BVH-bounded implicit molecular surface pass.
//!
//! One indirect fullscreen draw is recorded per surface representation. GPU
//! work scales with covered pixels, marching steps and visited hierarchy nodes;
//! no atom loop or surface mesh exists on the CPU frame path.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::visual_pipelines::{VisualPipelineSet, constants};
use crate::passes::{
    ALBEDO_RESOURCE, DEPTH_RESOURCE, ENTITY_RESOURCE, MOTION_RESOURCE, NORMAL_RESOURCE,
    STRUCTURE_RESOURCE, gbuffer_targets,
};
use molgfx_gpu::{
    ColorAttachment, CommandEncoder as _, CompareFunction, DepthAttachment, DepthLoadOp,
    DepthState, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
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
        if ctx.scene.surface_draws(false).next().is_none() {
            return;
        }
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "implicit molecular surfaces",
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
        for (group, args, shading) in ctx.scene.surface_draws(false) {
            if bound != Some(shading) {
                pass.set_pipeline(if shading.surface_grid() {
                    ctx.passes.surface.grid_surface.get(shading)
                } else {
                    ctx.passes.surface.union_surface.get(shading)
                });
                bound = Some(shading);
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
