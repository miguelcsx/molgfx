//! Analytic caller-authored primitive gbuffer pass.
//!
//! The table is sorted so each primitive class is one contiguous instance
//! range, so the pass changes pipeline at most once per class and draws each
//! range directly with its first-instance offset — no per-instance branching
//! and no rebinding between classes.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::primitive_pipelines::{
    OPAQUE_ENTRIES, PRIMITIVE_QUAD_VERTICES, PrimitivePipelineSet,
};
use crate::passes::{
    ALBEDO_RESOURCE, DEPTH_RESOURCE, ENTITY_RESOURCE, MOTION_RESOURCE, NORMAL_RESOURCE,
    STRUCTURE_RESOURCE, gbuffer_targets,
};
use pdviewx_gpu::{
    ColorAttachment, CommandEncoder as _, CompareFunction, DepthAttachment, DepthLoadOp,
    DepthState, Device, LoadOp, RenderPassDesc, RenderPassEncoder as _, TextureFormat,
};

#[derive(Debug)]
pub struct PrimitivePass<D: Device> {
    pipelines: PrimitivePipelineSet<D>,
}

impl<D: Device> PrimitivePass<D> {
    pub fn new(
        device: &D,
        group0: &D::BindGroupLayout,
        primitive: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        Ok(Self {
            pipelines: PrimitivePipelineSet::build(
                device,
                group0,
                None,
                primitive,
                &OPAQUE_ENTRIES,
                &gbuffer_targets(),
                Some(DepthState {
                    format: TextureFormat::Depth32Float,
                    write: true,
                    compare: CompareFunction::GreaterEqual,
                }),
            )?,
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
        let Some((table, runs)) = ctx.scene.primitive_groups() else {
            return;
        };
        if runs.iter().all(|run| run.translucent) {
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
        pass.set_bind_group(2, table, &[]);
        for run in runs.iter().filter(|run| !run.translucent) {
            let Some(pipeline) = ctx.passes.primitive.pipelines.pipeline(run) else {
                continue;
            };
            pass.set_pipeline(pipeline);
            pass.draw(0..PRIMITIVE_QUAD_VERTICES, run.first..run.first + run.len);
        }
    }
}

const fn attachment<D: Device>(view: &D::TextureView) -> ColorAttachment<'_, D> {
    ColorAttachment {
        view,
        load: LoadOp::Load,
    }
}
