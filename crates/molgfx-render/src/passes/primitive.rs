//! Analytic caller-authored primitive gbuffer pass.
//!
//! The table is sorted so each primitive class is one contiguous instance
//! range, so the pass changes pipeline at most once per class and draws each
//! range directly with its first-instance offset — no per-instance branching
//! and no rebinding between classes.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::ligand_pose_pipelines::LigandPosePipelineSet;
use crate::passes::primitive_pipelines::{
    OPAQUE_ENTRIES, PRIMITIVE_QUAD_VERTICES, PrimitivePipelineSet,
};
use crate::passes::visual_pipelines::{
    VISUAL_FRAGMENT_CONSTANT, VISUAL_PROGRAM_CONSTANT, VisualPipelineSet,
};
use crate::passes::{
    ALBEDO_RESOURCE, DEPTH_RESOURCE, ENTITY_RESOURCE, MOTION_RESOURCE, NORMAL_RESOURCE,
    STRUCTURE_RESOURCE, gbuffer_targets,
};
use crate::scene_gpu::{GENERIC_INSTANCE_CAPSULE, GENERIC_INSTANCE_SPHERE};
use molgfx_gpu::{
    ColorAttachment, CommandEncoder as _, CompareFunction, DepthAttachment, DepthLoadOp,
    DepthState, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, TextureFormat,
};

#[derive(Debug)]
pub(crate) struct PrimitivePass<D: Device> {
    pipelines: PrimitivePipelineSet<D>,
    ligand_pose: LigandPosePipelineSet<D>,
    generic_instance_sphere: VisualPipelineSet<D>,
    generic_instance_capsule: VisualPipelineSet<D>,
}

impl<D: Device> PrimitivePass<D> {
    pub(crate) fn new(
        device: &D,
        group0: &D::BindGroupLayout,
        primitive: &D::BindGroupLayout,
        ligand_pose: &D::BindGroupLayout,
        generic_instance: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let depth = Some(DepthState {
            format: TextureFormat::Depth32Float,
            write: true,
            compare: CompareFunction::GreaterEqual,
        });
        let generic_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "generic analytic instances",
            wgsl: molgfx_shaders::GENERIC_INSTANCE,
        })?;
        let generic_pipeline = |label, shape, visual, fragment| {
            device.create_render_pipeline(&RenderPipelineDesc {
                label,
                layouts: &[Some(group0), None, Some(generic_instance)],
                shader: &generic_shader,
                vs_entry: "vs_generic_instance",
                fs_entry: Some("fs_generic_instance"),
                color_targets: &gbuffer_targets(),
                depth,
                constants: &[
                    ("GENERIC_INSTANCE_SHAPE", f64::from(shape)),
                    ("PARTICLE_SHAPE_KIND", f64::from(shape)),
                    (VISUAL_PROGRAM_CONSTANT, if visual { 1.0 } else { 0.0 }),
                    (VISUAL_FRAGMENT_CONSTANT, if fragment { 1.0 } else { 0.0 }),
                ],
                topology: PrimitiveTopology::TriangleList,
            })
        };
        Ok(Self {
            pipelines: PrimitivePipelineSet::build(
                device,
                group0,
                None,
                primitive,
                &OPAQUE_ENTRIES,
                &gbuffer_targets(),
                depth,
            )?,
            ligand_pose: LigandPosePipelineSet::geometry(
                device,
                group0,
                None,
                ligand_pose,
                false,
                &gbuffer_targets(),
                depth,
            )?,
            generic_instance_sphere: VisualPipelineSet::new(
                generic_pipeline(
                    "generic analytic instance spheres",
                    GENERIC_INSTANCE_SPHERE,
                    false,
                    false,
                )?,
                generic_pipeline(
                    "generic analytic instance spheres with entity visual",
                    GENERIC_INSTANCE_SPHERE,
                    true,
                    false,
                )?,
                generic_pipeline(
                    "generic analytic instance spheres with fragment visual",
                    GENERIC_INSTANCE_SPHERE,
                    true,
                    true,
                )?,
            ),
            generic_instance_capsule: VisualPipelineSet::new(
                generic_pipeline(
                    "generic analytic instance capsules",
                    GENERIC_INSTANCE_CAPSULE,
                    false,
                    false,
                )?,
                generic_pipeline(
                    "generic analytic instance capsules with entity visual",
                    GENERIC_INSTANCE_CAPSULE,
                    true,
                    false,
                )?,
                generic_pipeline(
                    "generic analytic instance capsules with fragment visual",
                    GENERIC_INSTANCE_CAPSULE,
                    true,
                    true,
                )?,
            ),
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
        let primitives = ctx.scene.primitive_groups();
        let poses = ctx.scene.ligand_pose_draws();
        let has_generic = ctx.scene.generic_instance_draws(false).next().is_some();
        if primitives.is_none() && poses.is_none() && !has_generic {
            return;
        }
        let primitive_opaque =
            primitives.is_some_and(|(_, runs)| runs.iter().any(|run| !run.translucent));
        let pose_opaque = poses.is_some_and(|(_, _, runs)| runs.iter().any(|run| !run.translucent));
        if !primitive_opaque && !pose_opaque && !has_generic {
            return;
        }
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "analytic primitives",
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
        if let Some((table, runs)) = primitives {
            pass.set_bind_group(2, table, &[]);
            for run in runs.iter().filter(|run| !run.translucent) {
                let Some(pipeline) = ctx.passes.primitive.pipelines.pipeline(run) else {
                    continue;
                };
                pass.set_pipeline(pipeline);
                pass.draw(0..PRIMITIVE_QUAD_VERTICES, run.first..run.first + run.len);
            }
        }
        if let Some((table, args, runs)) = poses {
            pass.set_bind_group(2, table, &[]);
            for run in runs.iter().filter(|run| !run.translucent) {
                let Some(pipeline) = ctx.passes.primitive.ligand_pose.pipeline(run) else {
                    continue;
                };
                pass.set_pipeline(pipeline);
                pass.draw_indirect(args, run.args_offset);
            }
        }
        for draw in ctx.scene.generic_instance_draws(false) {
            let pipeline = if draw.shape == GENERIC_INSTANCE_SPHERE {
                ctx.passes
                    .primitive
                    .generic_instance_sphere
                    .get(draw.shading)
            } else {
                ctx.passes
                    .primitive
                    .generic_instance_capsule
                    .get(draw.shading)
            };
            pass.set_pipeline(pipeline);
            pass.set_bind_group(2, draw.group, &[]);
            pass.draw_indirect(draw.args, draw.args_offset);
        }
    }
}

const fn attachment<D: Device>(view: &D::TextureView) -> ColorAttachment<'_, D> {
    ColorAttachment {
        view,
        load: LoadOp::Load,
    }
}
