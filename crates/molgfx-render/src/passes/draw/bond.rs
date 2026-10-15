//! The analytic bond-capsule pass.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::visual_pipelines::{VisualPipelineSet, constants};
use crate::passes::{begin_gbuffer_pass, gbuffer_pipeline};
use molgfx_gpu::{Device, RenderPassEncoder as _, ShaderModuleDesc, TextureFormat};

#[derive(Debug)]
pub(crate) struct BondPass<D: Device> {
    capsule: VisualPipelineSet<D>,
    wire: VisualPipelineSet<D>,
    paged: D::Pipeline,
}

impl<D: Device> BondPass<D> {
    pub(crate) fn new(
        device: &D,
        _target_format: TextureFormat,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
        paged_layout: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "geometry_bond",
            wgsl: molgfx_shaders::GEOMETRY_BOND,
        })?;
        let pipeline = |label, vertex, fragment| {
            VisualPipelineSet::build(|set_constants| {
                gbuffer_pipeline(
                    device,
                    label,
                    &[Some(group0), None, Some(group2)],
                    &shader,
                    (vertex, fragment),
                    set_constants,
                )
            })
        };
        let paged_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "paged provider bonds",
            wgsl: molgfx_shaders::PAGED_BOND,
        })?;
        let paged = gbuffer_pipeline(
            device,
            "paged analytic bond capsules",
            &[Some(group0), Some(paged_layout)],
            &paged_shader,
            ("paged_bond_vertex", "paged_bond_fragment"),
            &[],
        )?;
        Ok(Self {
            capsule: pipeline("bond capsules", "vs_bond_capsule", "fs_bond_capsule")?,
            wire: pipeline("bond wires", "vs_bond_line", "fs_bond_line")?,
            paged,
        })
    }

    /// Compiles the bond pipeline built from the generated sibling.
    ///
    /// # Errors
    ///
    /// Shader compilation or pipeline creation failed.
    pub(crate) fn build_specialized(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
        wire: bool,
    ) -> Result<D::Pipeline, RenderError> {
        let (label, vertex, fragment) = if wire {
            ("specialized bond wires", "vs_bond_line", "fs_bond_line")
        } else {
            (
                "specialized bond capsules",
                "vs_bond_capsule",
                "fs_bond_capsule",
            )
        };
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "bond generated",
            wgsl: molgfx_shaders::GEOMETRY_BOND_SPECIALIZED,
        })?;
        Ok(gbuffer_pipeline(
            device,
            label,
            &[Some(group0), None, Some(group2)],
            &shader,
            (vertex, fragment),
            &constants(true),
        )?)
    }

    pub(crate) fn record(ctx: &mut PassContext<'_, D>) {
        if !ctx.scene.has_bond_draws(false) && ctx.scene.paged_bond_draw().is_none() {
            return;
        }
        let passes = ctx.passes;
        let Some(bond_pass) = ctx.build(&passes.bond, crate::passes::build::bond) else {
            return;
        };
        let Some(mut pass) = begin_gbuffer_pass(
            &mut *ctx.encoder,
            ctx.resources,
            ctx.timestamps,
            "bond capsules",
        ) else {
            return;
        };
        let mut bound: Option<*const D::Pipeline> = None;
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        if let Some(arena) = ctx.scene.indirect_args() {
            for (group2, offset, shading, specialized) in ctx.scene.bond_draws(false) {
                let pipeline = if shading.wire() {
                    bond_pass.wire.select(shading, specialized)
                } else {
                    bond_pass.capsule.select(shading, specialized)
                };
                if bound != Some(std::ptr::from_ref(pipeline)) {
                    pass.set_pipeline(pipeline);
                    bound = Some(std::ptr::from_ref(pipeline));
                }
                pass.set_bind_group(2, group2, &[]);
                pass.draw_indirect(arena, offset);
            }
        }
        if let Some((group, args)) = ctx.scene.paged_bond_draw() {
            pass.set_pipeline(&bond_pass.paged);
            pass.set_bind_group(1, group, &[]);
            pass.draw_indirect(args, 0);
        }
    }
}
