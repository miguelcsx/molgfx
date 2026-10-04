//! Weighted-blended order-independent translucent geometry.
//!
//! Work is `O(covered translucent pixels)` and draw submission is
//! `O(representations)`, independent of atom count.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::ligand_pose_pipelines::LigandPosePipelineSet;
use crate::passes::primitive_pipelines::PrimitivePipelineSet;
use crate::passes::visual_pipelines::VisualPipelineSet;
use crate::passes::{DEPTH_RESOURCE, FrameBindings, OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE};
use crate::scene_gpu::DrawFamily;
use crate::scene_gpu::{GENERIC_INSTANCE_CAPSULE, GENERIC_INSTANCE_SPHERE};
use molgfx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, ColorAttachment, CommandEncoder as _,
    DepthAttachment, DepthLoadOp, Device, LoadOp, RenderPassDesc, RenderPassEncoder as _,
    ShaderStages,
};

#[derive(Debug)]
pub(crate) struct OitPass<D: Device> {
    sphere: VisualPipelineSet<D>,
    sphere_clipped: VisualPipelineSet<D>,
    point: VisualPipelineSet<D>,
    generic_point: VisualPipelineSet<D>,
    bond: VisualPipelineSet<D>,
    wire: VisualPipelineSet<D>,
    cartoon: VisualPipelineSet<D>,
    union_surface: VisualPipelineSet<D>,
    grid_surface: VisualPipelineSet<D>,
    primitive: PrimitivePipelineSet<D>,
    ligand_pose: LigandPosePipelineSet<D>,
    generic_instance_sphere: VisualPipelineSet<D>,
    generic_instance_capsule: VisualPipelineSet<D>,
    volume: pipelines::VolumePipelineSet<D>,
    segmentation: pipelines::SegmentationPipelineSet<D>,
    segmentation_pick: pipelines::SegmentationPipelineSet<D>,
}

impl<D: Device> OitPass<D> {
    /// The opaque-scene inputs every transparent pipeline reads; the frame's
    /// bind groups are made over it whether or not the pipelines exist yet.
    pub(crate) fn layout(device: &D) -> D::BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group1: transparent opaque-scene inputs",
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture { filterable: true },
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::DepthTexture,
                },
            ],
        })
    }

    pub(crate) fn new(
        device: &D,
        layout: &D::BindGroupLayout,
        (group0, group2, ribbon): (
            &D::BindGroupLayout,
            &D::BindGroupLayout,
            &D::BindGroupLayout,
        ),
        analytic: (&D::BindGroupLayout, &D::BindGroupLayout),
        generic: (&D::BindGroupLayout, &D::BindGroupLayout),
        categorical: (
            &D::BindGroupLayout,
            &D::BindGroupLayout,
            &D::BindGroupLayout,
        ),
    ) -> Result<Self, RenderError> {
        let (sphere, sphere_clipped) = sphere_pipelines(device, group0, layout, group2)?;
        let (union_surface, grid_surface) = surface_pipelines(device, group0, layout, group2)?;
        let (generic_instance_sphere, generic_instance_capsule) =
            generic_instance_pipelines(device, group0, layout, generic.1)?;
        Ok(Self {
            sphere,
            sphere_clipped,
            point: visual_pipeline(
                device,
                group0,
                layout,
                &OitPipelineDesc {
                    label: "transparent atom points",
                    wgsl: molgfx_shaders::GEOMETRY_POINT,
                    vertex: "vs_point_transparent",
                    fragment: "fs_point_transparent",
                    group2,
                },
            )?,
            generic_point: visual_pipeline(
                device,
                group0,
                layout,
                &OitPipelineDesc {
                    label: "transparent generic analytic points",
                    wgsl: molgfx_shaders::GENERIC_POINT,
                    vertex: "vs_generic_point",
                    fragment: "fs_generic_point_transparent",
                    group2: generic.0,
                },
            )?,
            bond: visual_pipeline(
                device,
                group0,
                layout,
                &OitPipelineDesc {
                    label: "transparent bond capsules",
                    wgsl: molgfx_shaders::GEOMETRY_BOND,
                    vertex: "vs_bond_capsule",
                    fragment: "fs_bond_capsule_transparent",
                    group2,
                },
            )?,
            wire: visual_pipeline(
                device,
                group0,
                layout,
                &OitPipelineDesc {
                    label: "transparent bond wires",
                    wgsl: molgfx_shaders::GEOMETRY_BOND,
                    vertex: "vs_bond_line",
                    fragment: "fs_bond_line_transparent",
                    group2,
                },
            )?,
            cartoon: visual_pipeline(
                device,
                group0,
                layout,
                &OitPipelineDesc {
                    label: "transparent cartoon ribbons",
                    wgsl: molgfx_shaders::GEOMETRY_CARTOON,
                    vertex: "vs_cartoon",
                    fragment: "fs_cartoon_transparent",
                    group2: ribbon,
                },
            )?,
            union_surface,
            grid_surface,
            primitive: primitive_pipelines(device, group0, layout, analytic.0)?,
            ligand_pose: LigandPosePipelineSet::geometry(
                device,
                group0,
                Some(layout),
                analytic.1,
                true,
                &pipelines::oit_targets(),
                Some(pipelines::oit_depth()),
            )?,
            generic_instance_sphere,
            generic_instance_capsule,
            volume: volume_pipelines(device, group0, layout, categorical.0, categorical.2)?,
            segmentation: segmentation_pipelines(
                device,
                group0,
                layout,
                categorical.1,
                categorical.2,
                pipelines::SegmentationPass::Beauty,
            )?,
            segmentation_pick: segmentation_pipelines(
                device,
                group0,
                layout,
                categorical.1,
                categorical.2,
                pipelines::SegmentationPass::Identity,
            )?,
        })
    }

    pub(crate) fn clear(ctx: &mut PassContext<'_, D>) {
        if !ctx.scene.has_translucency() {
            return;
        }
        let (Some(accumulation), Some(revealage)) = (
            ctx.resources.view(OIT_ACCUM_RESOURCE),
            ctx.resources.view(OIT_REVEAL_RESOURCE),
        ) else {
            return;
        };
        let _pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "clear transparency accumulation",
            colors: &[
                ColorAttachment {
                    view: accumulation,
                    load: LoadOp::Clear([0.0; 4]),
                },
                ColorAttachment {
                    view: revealage,
                    load: LoadOp::Clear([1.0; 4]),
                },
            ],
            depth: None,
            timestamps: ctx.timestamps,
        });
    }

    pub(crate) fn spheres(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.atom_draws(true).next().is_some() {
            record(ctx, Primitive::Spheres);
        }
    }

    pub(crate) fn bonds(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.bond_draws(true).next().is_some() {
            record(ctx, Primitive::Bonds);
        }
    }

    pub(crate) fn points(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.point_draws(true).next().is_some()
            || ctx.scene.generic_point_draws(true).next().is_some()
        {
            record(ctx, Primitive::Points);
        }
    }

    pub(crate) fn cartoons(ctx: &mut PassContext<'_, D>) {
        if ctx
            .scene
            .cartoon_draws(true, DrawFamily::Cartoon)
            .next()
            .is_some()
            || ctx.scene.mesh_draws(true).next().is_some()
        {
            record(ctx, Primitive::Cartoons);
        }
    }

    pub(crate) fn surfaces(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.surface_draws(true).next().is_some() {
            record(ctx, Primitive::Surfaces);
        }
    }

    pub(crate) fn primitive(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.has_transparent_primitives() {
            record(ctx, Primitive::Analytic);
        }
    }

    pub(crate) fn volumes(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.volume_draws().next().is_some() {
            record(ctx, Primitive::Volumes);
        }
    }
}

#[derive(Clone, Copy)]
enum Primitive {
    Spheres,
    Points,
    Bonds,
    Cartoons,
    Surfaces,
    Analytic,
    Volumes,
}

fn record<D: Device>(ctx: &mut PassContext<'_, D>, primitive: Primitive) {
    let (Some(accumulation), Some(revealage), Some(depth)) = (
        ctx.resources.view(OIT_ACCUM_RESOURCE),
        ctx.resources.view(OIT_REVEAL_RESOURCE),
        ctx.resources.view(DEPTH_RESOURCE),
    ) else {
        return;
    };
    let Some(FrameBindings { oit, .. }) = ctx.bindings else {
        return;
    };
    let passes = ctx.passes;
    let Some(oit_pass) = ctx.build(&passes.oit, |env| {
        super::build::oit(env, &passes.oit_layout)
    }) else {
        return;
    };
    let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
        label: "weighted blended transparent geometry",
        colors: &[
            ColorAttachment {
                view: accumulation,
                load: LoadOp::Load,
            },
            ColorAttachment {
                view: revealage,
                load: LoadOp::Load,
            },
        ],
        depth: Some(DepthAttachment {
            view: depth,
            load: DepthLoadOp::Load,
            read_only: true,
        }),
        timestamps: ctx.timestamps,
    });
    pass.set_bind_group(0, &ctx.scene.group0, &[]);
    pass.set_bind_group(1, oit, &[]);
    match primitive {
        Primitive::Spheres => record_spheres(oit_pass, ctx.scene, &mut pass),
        Primitive::Bonds => record_bonds(oit_pass, ctx.scene, &mut pass),
        Primitive::Points => {
            record_points(oit_pass, ctx.scene, &mut pass);
            record_generic_points(oit_pass, ctx.scene, &mut pass);
        }
        Primitive::Cartoons => {
            let mut bound = None;
            for draw in ctx
                .scene
                .cartoon_draws(true, DrawFamily::Cartoon)
                .chain(ctx.scene.mesh_draws(true))
            {
                if bound != Some(draw.shading) {
                    pass.set_pipeline(oit_pass.cartoon.get(draw.shading));
                    bound = Some(draw.shading);
                }
                pass.set_bind_group(2, draw.group, &[]);
                pass.set_index_buffer(draw.indices, molgfx_gpu::IndexFormat::Uint32);
                pass.draw_indexed_indirect(draw.arguments, 0);
            }
        }
        Primitive::Surfaces => record_surfaces(oit_pass, ctx.scene, &mut pass),
        Primitive::Analytic => {
            record_primitives(oit_pass, ctx.scene, &mut pass);
            record_generic_instances(oit_pass, ctx.scene, &mut pass);
        }
        Primitive::Volumes => {
            let mut current = None;
            for (rendering, group, geometry) in ctx.scene.volume_draws() {
                if current != Some((rendering, geometry.kind())) {
                    pass.set_pipeline(oit_pass.volume.get(rendering, geometry.kind()));
                    current = Some((rendering, geometry.kind()));
                }
                pass.set_bind_group(2, group, &[]);
                match geometry {
                    crate::scene_gpu::FieldGeometry::Proxy => pass.draw(0..6, 0..1),
                    crate::scene_gpu::FieldGeometry::Boundary(boundary, arguments) => {
                        pass.set_bind_group(3, boundary, &[]);
                        pass.draw_indirect(arguments, 0);
                    }
                }
            }
        }
    }
}

mod pipelines;
mod record;

use pipelines::{
    OitPipelineDesc, primitive_pipelines, segmentation_pipelines, visual_pipeline, volume_pipelines,
};
use pipelines::{generic_instance_pipelines, sphere_pipelines, surface_pipelines};
use record::{record_primitives, record_spheres, record_surfaces};

fn record_bonds<D: Device, P: molgfx_gpu::RenderPassEncoder<D>>(
    oit: &OitPass<D>,
    scene: &crate::scene_gpu::GpuScene<D>,
    pass: &mut P,
) {
    let mut bound = None;
    if let Some(arena) = scene.indirect_args() {
        for (group, offset, shading, _) in scene.bond_draws(true) {
            if bound != Some(shading) {
                pass.set_pipeline(if shading.wire() {
                    oit.wire.get(shading)
                } else {
                    oit.bond.get(shading)
                });
                bound = Some(shading);
            }
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(arena, offset);
        }
    }
}

fn record_points<D: Device, P: molgfx_gpu::RenderPassEncoder<D>>(
    oit: &OitPass<D>,
    scene: &crate::scene_gpu::GpuScene<D>,
    pass: &mut P,
) {
    let mut bound = None;
    if let Some(arena) = scene.indirect_args() {
        for (group, offset, shading, _) in scene.point_draws(true) {
            if bound != Some(shading) {
                pass.set_pipeline(oit.point.get(shading));
                bound = Some(shading);
            }
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(arena, offset);
        }
    }
}

fn record_generic_points<D: Device, P: molgfx_gpu::RenderPassEncoder<D>>(
    oit: &OitPass<D>,
    scene: &crate::scene_gpu::GpuScene<D>,
    pass: &mut P,
) {
    let mut bound = None;
    for (group, args, shading) in scene.generic_point_draws(true) {
        if bound != Some(shading) {
            pass.set_pipeline(oit.generic_point.get(shading));
            bound = Some(shading);
        }
        pass.set_bind_group(2, group, &[]);
        pass.draw_indirect(args, 0);
    }
}

fn record_generic_instances<D: Device, P: molgfx_gpu::RenderPassEncoder<D>>(
    oit: &OitPass<D>,
    scene: &crate::scene_gpu::GpuScene<D>,
    pass: &mut P,
) {
    for draw in scene.generic_instance_draws(true) {
        let pipelines = if draw.shape == GENERIC_INSTANCE_SPHERE {
            &oit.generic_instance_sphere
        } else if draw.shape == GENERIC_INSTANCE_CAPSULE {
            &oit.generic_instance_capsule
        } else {
            continue;
        };
        pass.set_pipeline(pipelines.get(draw.shading));
        pass.set_bind_group(2, draw.group, &[]);
        pass.draw_indirect(draw.args, draw.args_offset);
    }
}

pub(crate) mod segmentation;
