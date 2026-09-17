//! The analytic bond-capsule pass.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::DEPTH_RESOURCE;
use crate::passes::visual_pipelines::{VisualPipelineSet, constants};
use crate::passes::{
    ALBEDO_RESOURCE, ENTITY_RESOURCE, MOTION_RESOURCE, NORMAL_RESOURCE, STRUCTURE_RESOURCE,
    gbuffer_targets,
};
use molgfx_gpu::{
    ColorAttachment, CommandEncoder as _, CompareFunction, DepthAttachment, DepthLoadOp,
    DepthState, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, TextureFormat,
};

#[derive(Debug)]
pub struct BondPass<D: Device> {
    capsule: VisualPipelineSet<D>,
    wire: VisualPipelineSet<D>,
    paged: D::Pipeline,
}

impl<D: Device> BondPass<D> {
    pub fn new(
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
        let pipeline = |label, vertex, fragment| -> Result<VisualPipelineSet<D>, RenderError> {
            let build = |pipeline_constants: &[(&'static str, f64)]| {
                device.create_render_pipeline(&RenderPipelineDesc {
                    label,
                    layouts: &[Some(group0), None, Some(group2)],
                    shader: &shader,
                    vs_entry: vertex,
                    fs_entry: Some(fragment),
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
            label: "paged provider bonds",
            wgsl: molgfx_shaders::PAGED_BOND,
        })?;
        let paged = device.create_render_pipeline(&RenderPipelineDesc {
            label: "paged analytic bond capsules",
            layouts: &[Some(group0), Some(paged_layout)],
            shader: &paged_shader,
            vs_entry: "paged_bond_vertex",
            fs_entry: Some("paged_bond_fragment"),
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
            capsule: pipeline("bond capsules", "vs_bond_capsule", "fs_bond_capsule")?,
            wire: pipeline("bond wires", "vs_bond_line", "fs_bond_line")?,
            paged,
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
        if ctx.scene.bond_draws(false).next().is_none() && ctx.scene.paged_bond_draw().is_none() {
            return;
        }
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "bond capsules",
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
        let mut bound = None;
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        for (group2, args, shading) in ctx.scene.bond_draws(false) {
            if bound != Some(shading) {
                pass.set_pipeline(if shading.wire() {
                    ctx.passes.bond.wire.get(shading)
                } else {
                    ctx.passes.bond.capsule.get(shading)
                });
                bound = Some(shading);
            }
            pass.set_bind_group(2, group2, &[]);
            pass.draw_indirect(args, 0);
        }
        if let Some((group, args)) = ctx.scene.paged_bond_draw() {
            pass.set_pipeline(&ctx.passes.bond.paged);
            pass.set_bind_group(1, group, &[]);
            pass.draw_indirect(args, 0);
        }
    }
}
