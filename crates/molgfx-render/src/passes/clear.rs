//! Clears the shared opaque gbuffer and reversed depth in one attachment pass.

use crate::graph::PassContext;
use crate::passes::{
    ALBEDO_RESOURCE, DEPTH_RESOURCE, ENTITY_RESOURCE, MOTION_RESOURCE, NORMAL_RESOURCE,
    SEGMENT_LABEL_RESOURCE, SEGMENT_VOLUME_RESOURCE, STRUCTURE_RESOURCE,
};
use molgfx_gpu::{
    ColorAttachment, CommandEncoder as _, DepthAttachment, DepthLoadOp, Device, LoadOp,
    RenderPassDesc,
};
/// Load-time state of the clear pass.
#[derive(Debug)]
pub(crate) struct ClearPass;

impl ClearPass {
    /// Records the pass: one fullscreen triangle into the frame target.
    pub(crate) fn record<D: Device>(ctx: &mut PassContext<'_, D>) {
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
        let _pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "clear gbuffer",
            colors: &[
                ColorAttachment {
                    view: albedo,
                    load: LoadOp::Clear([0.0, 0.0, 0.0, 0.0]),
                },
                ColorAttachment {
                    view: normal,
                    load: LoadOp::Clear([0.0, 0.0, 1.0, 1.0]),
                },
                ColorAttachment {
                    view: entity,
                    load: LoadOp::Clear([4_294_967_295.0, 0.0, 0.0, 0.0]),
                },
                ColorAttachment {
                    view: structure,
                    load: LoadOp::Clear([4_294_967_295.0, 0.0, 0.0, 0.0]),
                },
                ColorAttachment {
                    view: motion,
                    load: LoadOp::Clear([0.0, 0.0, 0.0, 0.0]),
                },
            ],
            depth: Some(DepthAttachment {
                view: depth,
                load: DepthLoadOp::Clear(0.0),
                read_only: false,
            }),
            timestamps: ctx.timestamps,
        });
    }

    /// Clears the categorical picking attachments to their shared empty
    /// sentinel before any transparent segment draw.
    pub(crate) fn segment_ids<D: Device>(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.is_massive_points_only() {
            return;
        }
        let (Some(volume), Some(label)) = (
            ctx.resources.view(SEGMENT_VOLUME_RESOURCE),
            ctx.resources.view(SEGMENT_LABEL_RESOURCE),
        ) else {
            return;
        };
        let _pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "clear categorical segment ids",
            colors: &[
                ColorAttachment {
                    view: volume,
                    load: LoadOp::Clear([4_294_967_295.0, 0.0, 0.0, 0.0]),
                },
                ColorAttachment {
                    view: label,
                    load: LoadOp::Clear([4_294_967_295.0, 0.0, 0.0, 0.0]),
                },
            ],
            depth: None,
            timestamps: ctx.timestamps,
        });
    }
}
