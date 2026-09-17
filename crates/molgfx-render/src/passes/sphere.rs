//! The sphere impostor pass.
//!
//! Draws every packed atom as one instanced quad through a single indirect
//! draw whose arguments live GPU-side; the CPU records one call regardless
//! of atom count. Per-frame GPU cost is `O(covered pixels)` — the impostor
//! path is fill-bound, which is why the quads bound their spheres tightly.

use crate::error::RenderError;
use crate::graph::{PassContext, ResourceId};
use crate::passes::visual_pipelines::{constants, VisualPipelineSet};
use crate::passes::{
    gbuffer_targets, ALBEDO_RESOURCE, ENTITY_RESOURCE, MOTION_RESOURCE, NORMAL_RESOURCE,
    STRUCTURE_RESOURCE,
};
use molgfx_gpu::{
    ColorAttachment, CommandEncoder as _, CompareFunction, DepthAttachment, DepthLoadOp,
    DepthState, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, TextureFormat,
};

/// The graph resource id of the frame depth buffer.
pub const DEPTH_RESOURCE: ResourceId = ResourceId(0);

/// Load-time state of the sphere pass.
///
/// The clipped and unclipped fragment paths are separate pipelines rather than
/// one shader branching per fragment, so a representation without clip planes
/// never evaluates a clip test or clip-cap material on any covered pixel.
#[derive(Debug)]
pub struct SpherePass<D: Device> {
    unclipped: VisualPipelineSet<D>,
    clipped: VisualPipelineSet<D>,
    paged: D::Pipeline,
}

impl<D: Device> SpherePass<D> {
    /// Compiles the sphere pipeline against the presentation format.
    ///
    /// # Errors
    ///
    /// Shader compilation or pipeline creation failed.
    pub fn new(
        device: &D,
        _target_format: TextureFormat,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
        paged_layout: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "geometry_sphere",
            wgsl: molgfx_shaders::GEOMETRY_SPHERE,
        })?;
        // Groups: 0 per-frame camera, 1 unused, 2 per-representation atoms.
        let pipeline = |label, fs_entry| -> Result<VisualPipelineSet<D>, RenderError> {
            let build = |pipeline_constants: &[(&'static str, f64)]| {
                device.create_render_pipeline(&RenderPipelineDesc {
                    label,
                    layouts: &[Some(group0), None, Some(group2)],
                    shader: &shader,
                    vs_entry: "vs_sphere_opaque",
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
        let paged_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "paged structure chunks",
            wgsl: molgfx_shaders::PAGED_CHUNK,
        })?;
        let paged = device.create_render_pipeline(&RenderPipelineDesc {
            label: "paged spacefill spheres",
            layouts: &[Some(group0), Some(paged_layout)],
            shader: &paged_shader,
            vs_entry: "paged_spacefill_vertex",
            fs_entry: Some("paged_spacefill_fragment"),
            color_targets: &gbuffer_targets(),
            depth: Some(DepthState {
                format: TextureFormat::Depth32Float,
                write: true,
                compare: CompareFunction::GreaterEqual,
            }),
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        Ok(Self {
            unclipped: pipeline("sphere impostors", "fs_sphere")?,
            clipped: pipeline("clipped sphere impostors", "fs_sphere_clipped")?,
            paged,
        })
    }

    /// Records one indirect draw per ordered representation slot.
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
        if ctx.scene.atom_draws(false).next().is_none()
            && ctx.scene.paged_spacefill_draw().is_none()
        {
            return;
        }
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "sphere impostors",
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
        // Draws arrive grouped by representation, so the pipeline changes at
        // most once per representation rather than once per draw.
        let mut bound = None;
        for (group2, args, shading) in ctx.scene.atom_draws(false) {
            if bound != Some(shading) {
                pass.set_pipeline(if shading.clipped() {
                    ctx.passes.sphere.clipped.get(shading)
                } else {
                    ctx.passes.sphere.unclipped.get(shading)
                });
                bound = Some(shading);
            }
            pass.set_bind_group(2, group2, &[]);
            pass.draw_indirect(args, 0);
        }
        if let Some((group, args, offset)) = ctx.scene.paged_spacefill_draw() {
            pass.set_pipeline(&ctx.passes.sphere.paged);
            pass.set_bind_group(1, group, &[]);
            pass.draw_indirect(args, offset);
        }
    }
}
