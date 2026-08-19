//! Weighted-blended order-independent translucent geometry.
//!
//! Work is `O(covered translucent pixels)` and draw submission is
//! `O(representations)`, independent of atom count.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::primitive_pipelines::PrimitivePipelineSet;
use crate::passes::{
    DEPTH_RESOURCE, FrameBindings, OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE, SEGMENT_LABEL_RESOURCE,
    SEGMENT_VOLUME_RESOURCE,
};
use pdviewx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, ColorAttachment, CommandEncoder as _,
    DepthAttachment, DepthLoadOp, Device, LoadOp, RenderPassDesc, RenderPassEncoder as _,
    ShaderStages,
};

#[derive(Debug)]
pub struct OitPass<D: Device> {
    sphere: D::Pipeline,
    sphere_clipped: D::Pipeline,
    point: D::Pipeline,
    bond: D::Pipeline,
    wire: D::Pipeline,
    cartoon: D::Pipeline,
    union_surface: D::Pipeline,
    grid_surface: D::Pipeline,
    primitive: PrimitivePipelineSet<D>,
    volume: D::Pipeline,
    segmentation: D::Pipeline,
    pub(crate) layout: D::BindGroupLayout,
}

impl<D: Device> OitPass<D> {
    pub fn new(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
        ribbon: &D::BindGroupLayout,
        primitive: &D::BindGroupLayout,
        volume: &D::BindGroupLayout,
        segmentation: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
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
        });
        let (sphere, sphere_clipped) = sphere_pipelines(device, group0, &layout, group2)?;
        let (union_surface, grid_surface) = surface_pipelines(device, group0, &layout, group2)?;
        Ok(Self {
            sphere,
            sphere_clipped,
            point: pipeline(
                device,
                group0,
                &layout,
                &OitPipelineDesc {
                    label: "transparent atom points",
                    wgsl: pdviewx_shaders::GEOMETRY_POINT,
                    vertex: "vs_point_transparent",
                    fragment: "fs_point_transparent",
                    group2,
                },
            )?,
            bond: pipeline(
                device,
                group0,
                &layout,
                &OitPipelineDesc {
                    label: "transparent bond capsules",
                    wgsl: pdviewx_shaders::GEOMETRY_BOND,
                    vertex: "vs_bond_capsule",
                    fragment: "fs_bond_capsule_transparent",
                    group2,
                },
            )?,
            wire: pipeline(
                device,
                group0,
                &layout,
                &OitPipelineDesc {
                    label: "transparent bond wires",
                    wgsl: pdviewx_shaders::GEOMETRY_BOND,
                    vertex: "vs_bond_line",
                    fragment: "fs_bond_line_transparent",
                    group2,
                },
            )?,
            cartoon: pipeline(
                device,
                group0,
                &layout,
                &OitPipelineDesc {
                    label: "transparent cartoon ribbons",
                    wgsl: pdviewx_shaders::GEOMETRY_CARTOON,
                    vertex: "vs_cartoon",
                    fragment: "fs_cartoon_transparent",
                    group2: ribbon,
                },
            )?,
            union_surface,
            grid_surface,
            primitive: primitive_pipelines(device, group0, &layout, primitive)?,
            volume: pipeline(
                device,
                group0,
                &layout,
                &OitPipelineDesc {
                    label: "direct density volumes",
                    wgsl: pdviewx_shaders::VOLUME,
                    vertex: "vs_volume",
                    fragment: "fs_volume",
                    group2: volume,
                },
            )?,
            segmentation: segmentation_pipeline(device, group0, &layout, segmentation)?,
            layout,
        })
    }

    pub fn clear(ctx: &mut PassContext<'_, D>) {
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

    pub fn spheres(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.atom_draws(true).next().is_some() {
            record(ctx, Primitive::Spheres);
        }
    }

    pub fn bonds(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.bond_draws(true).next().is_some() {
            record(ctx, Primitive::Bonds);
        }
    }

    pub fn points(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.point_draws(true).next().is_some() {
            record(ctx, Primitive::Points);
        }
    }

    pub fn cartoons(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.cartoon_draws(true).next().is_some()
            || ctx.scene.mesh_draws(true).next().is_some()
        {
            record(ctx, Primitive::Cartoons);
        }
    }

    pub fn surfaces(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.surface_draws(true).next().is_some() {
            record(ctx, Primitive::Surfaces);
        }
    }

    pub fn primitive(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.has_transparent_primitives() {
            record(ctx, Primitive::Analytic);
        }
    }

    pub fn volumes(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.volume_draws().next().is_some() {
            record(ctx, Primitive::Volumes);
        }
    }

    pub fn segmentations(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.segmentation_draws().next().is_none() {
            return;
        }
        let (
            Some(accumulation),
            Some(revealage),
            Some(segment_volume),
            Some(segment_label),
            Some(depth),
        ) = (
            ctx.resources.view(OIT_ACCUM_RESOURCE),
            ctx.resources.view(OIT_REVEAL_RESOURCE),
            ctx.resources.view(SEGMENT_VOLUME_RESOURCE),
            ctx.resources.view(SEGMENT_LABEL_RESOURCE),
            ctx.resources.view(DEPTH_RESOURCE),
        )
        else {
            return;
        };
        let Some(FrameBindings { oit, .. }) = ctx.bindings else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "categorical segmentation volumes",
            colors: &[
                ColorAttachment {
                    view: accumulation,
                    load: LoadOp::Load,
                },
                ColorAttachment {
                    view: revealage,
                    load: LoadOp::Load,
                },
                ColorAttachment {
                    view: segment_volume,
                    load: LoadOp::Load,
                },
                ColorAttachment {
                    view: segment_label,
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
        pass.set_pipeline(&ctx.passes.oit.segmentation);
        for group in ctx.scene.segmentation_draws() {
            pass.set_bind_group(2, group, &[]);
            pass.draw(0..6, 0..1);
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
        Primitive::Spheres => record_spheres(ctx.passes, ctx.scene, &mut pass),
        Primitive::Bonds => {
            let mut bound = None;
            for (group, args, shading) in ctx.scene.bond_draws(true) {
                if bound != Some(shading.wire) {
                    pass.set_pipeline(if shading.wire {
                        &ctx.passes.oit.wire
                    } else {
                        &ctx.passes.oit.bond
                    });
                    bound = Some(shading.wire);
                }
                pass.set_bind_group(2, group, &[]);
                pass.draw_indirect(args, 0);
            }
        }
        Primitive::Points => {
            pass.set_pipeline(&ctx.passes.oit.point);
            for (group, args, _) in ctx.scene.point_draws(true) {
                pass.set_bind_group(2, group, &[]);
                pass.draw_indirect(args, 0);
            }
        }
        Primitive::Cartoons => {
            pass.set_pipeline(&ctx.passes.oit.cartoon);
            for (group, args, _) in ctx
                .scene
                .cartoon_draws(true)
                .chain(ctx.scene.mesh_draws(true))
            {
                pass.set_bind_group(2, group, &[]);
                pass.draw_indirect(args, 0);
            }
        }
        Primitive::Surfaces => record_surfaces(ctx.passes, ctx.scene, &mut pass),
        Primitive::Analytic => record_primitives(ctx.passes, ctx.scene, &mut pass),
        Primitive::Volumes => {
            pass.set_pipeline(&ctx.passes.oit.volume);
            for group in ctx.scene.volume_draws() {
                pass.set_bind_group(2, group, &[]);
                pass.draw(0..6, 0..1);
            }
        }
    }
}

mod pipelines;
mod record;

use pipelines::{OitPipelineDesc, pipeline, primitive_pipelines, segmentation_pipeline};
use pipelines::{sphere_pipelines, surface_pipelines};
use record::{record_primitives, record_spheres, record_surfaces};
