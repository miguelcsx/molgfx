//! The sphere impostor pass.
//!
//! Draws every packed atom as one instanced quad through a single indirect
//! draw whose arguments live GPU-side; the CPU records one call regardless
//! of atom count. Per-frame GPU cost is `O(covered pixels)` — the impostor
//! path is fill-bound, which is why the quads bound their spheres tightly.

use crate::error::RenderError;
use crate::graph::{PassContext, ResourceId};
use crate::passes::visual_pipelines::{VisualPipelineSet, constants};
use crate::passes::{begin_gbuffer_pass, gbuffer_pipeline};
use molgfx_gpu::{Device, RenderPassEncoder as _, ShaderModuleDesc, TextureFormat};

/// The graph resource id of the frame depth buffer.
pub(crate) const DEPTH_RESOURCE: ResourceId = ResourceId(0);

/// Load-time state of the sphere pass.
///
/// The clipped and unclipped fragment paths are separate pipelines rather than
/// one shader branching per fragment, so a representation without clip planes
/// never evaluates a clip test or clip-cap material on any covered pixel.
#[derive(Debug)]
pub(crate) struct SpherePass<D: Device> {
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
    pub(crate) fn new(
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
        let pipeline = |label, fs_entry| {
            VisualPipelineSet::build(|set_constants| {
                gbuffer_pipeline(
                    device,
                    label,
                    &[Some(group0), None, Some(group2)],
                    &shader,
                    ("vs_sphere_opaque", fs_entry),
                    set_constants,
                )
            })
        };
        let paged_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "paged structure chunks",
            wgsl: molgfx_shaders::PAGED_CHUNK,
        })?;
        let paged = gbuffer_pipeline(
            device,
            "paged spacefill spheres",
            &[Some(group0), Some(paged_layout)],
            &paged_shader,
            ("paged_spacefill_vertex", "paged_spacefill_fragment"),
            &[],
        )?;
        Ok(Self {
            unclipped: pipeline("sphere impostors", "fs_sphere")?,
            clipped: pipeline("clipped sphere impostors", "fs_sphere_clipped")?,
            paged,
        })
    }

    /// Compiles the sphere impostor pipeline built from the generated sibling.
    ///
    /// The sibling keeps every interpreted entry, so the generated unit is
    /// selected by the same fragment shading as the interpreted one.
    ///
    /// # Errors
    ///
    /// Shader compilation or pipeline creation failed.
    pub(crate) fn build_specialized(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
        clipped: bool,
    ) -> Result<D::Pipeline, RenderError> {
        let (label, fs_entry) = if clipped {
            ("specialized clipped sphere impostors", "fs_sphere_clipped")
        } else {
            ("specialized sphere impostors", "fs_sphere")
        };
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "sphere impostors generated",
            wgsl: molgfx_shaders::GEOMETRY_SPHERE_SPECIALIZED,
        })?;
        Ok(gbuffer_pipeline(
            device,
            label,
            &[Some(group0), None, Some(group2)],
            &shader,
            ("vs_sphere_opaque", fs_entry),
            &constants(true),
        )?)
    }

    /// Records one indirect draw per ordered representation slot.
    pub(crate) fn record(ctx: &mut PassContext<'_, D>) {
        if !ctx.scene.has_atom_draws(false) && ctx.scene.paged_spacefill_draw().is_none() {
            return;
        }
        let passes = ctx.passes;
        let Some(sphere) = ctx.build(&passes.sphere, crate::passes::build::sphere) else {
            return;
        };
        let Some(mut pass) = begin_gbuffer_pass(
            &mut *ctx.encoder,
            ctx.resources,
            ctx.timestamps,
            "sphere impostors",
        ) else {
            return;
        };
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        // Draws arrive grouped by representation, so the pipeline changes at
        // most once per representation rather than once per draw.
        let mut bound: Option<*const D::Pipeline> = None;
        if let Some(arena) = ctx.scene.indirect_args() {
            for (group2, offset, shading, specialized) in ctx.scene.atom_draws(false) {
                let pipeline = if shading.clipped() {
                    sphere.clipped.select(shading, specialized)
                } else {
                    sphere.unclipped.select(shading, specialized)
                };
                if bound != Some(std::ptr::from_ref(pipeline)) {
                    pass.set_pipeline(pipeline);
                    bound = Some(std::ptr::from_ref(pipeline));
                }
                pass.set_bind_group(2, group2, &[]);
                pass.draw_indirect(arena, offset);
            }
        }
        if let Some((group, args, offset)) = ctx.scene.paged_spacefill_draw() {
            pass.set_pipeline(&sphere.paged);
            pass.set_bind_group(1, group, &[]);
            pass.draw_indirect(args, offset);
        }
    }
}
