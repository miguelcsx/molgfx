//! Categorical optical and indexed boundary draw recording.
use crate::graph::PassContext;
use crate::passes::{
    DEPTH_RESOURCE, FrameBindings, OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE, SEGMENT_LABEL_RESOURCE,
    SEGMENT_VOLUME_RESOURCE,
};
use molgfx_gpu::{
    ColorAttachment, CommandEncoder as _, DepthAttachment, DepthLoadOp, Device, LoadOp,
    RenderPassDesc, RenderPassEncoder as _,
};

pub(crate) fn record<D: Device>(ctx: &mut PassContext<'_, D>) {
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
    let passes = ctx.passes;
    let Some(oit_pass) = ctx.build(&passes.oit, |env| {
        crate::passes::build::oit(env, &passes.oit_layout)
    }) else {
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
    let mut current = None;
    for (key, group, geometry) in ctx.scene.segmentation_draws() {
        if current != Some(key) {
            pass.set_pipeline(oit_pass.segmentation.get(key));
            current = Some(key);
        }
        pass.set_bind_group(2, group, &[]);
        match geometry {
            crate::scene_gpu::SegmentationGeometry::Proxy => pass.draw(0..6, 0..1),
            crate::scene_gpu::SegmentationGeometry::Boundary(group, arguments) => {
                pass.set_bind_group(3, group, &[]);
                pass.draw_indirect(arguments, 0);
            }
        }
    }
}

pub(crate) fn record_identity<D: Device>(ctx: &mut PassContext<'_, D>) {
    if ctx.scene.segmentation_draws().next().is_none() {
        return;
    }
    let (Some(segment_volume), Some(segment_label), Some(depth)) = (
        ctx.resources.view(SEGMENT_VOLUME_RESOURCE),
        ctx.resources.view(SEGMENT_LABEL_RESOURCE),
        ctx.resources.view(crate::passes::SEGMENT_DEPTH_RESOURCE),
    ) else {
        return;
    };
    let Some(FrameBindings { oit, .. }) = ctx.bindings else {
        return;
    };
    let passes = ctx.passes;
    let Some(oit_pass) = ctx.build(&passes.oit, |env| {
        crate::passes::build::oit(env, &passes.oit_layout)
    }) else {
        return;
    };
    let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
        label: "nearest categorical identities",
        colors: &[
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
            read_only: false,
        }),
        timestamps: ctx.timestamps,
    });
    pass.set_bind_group(0, &ctx.scene.group0, &[]);
    pass.set_bind_group(1, oit, &[]);
    let mut current = None;
    for (key, group, geometry) in ctx.scene.segmentation_draws() {
        if current != Some(key) {
            pass.set_pipeline(oit_pass.segmentation_pick.get(key));
            current = Some(key);
        }
        pass.set_bind_group(2, group, &[]);
        match geometry {
            crate::scene_gpu::SegmentationGeometry::Proxy => pass.draw(0..6, 0..1),
            crate::scene_gpu::SegmentationGeometry::Boundary(group, arguments) => {
                pass.set_bind_group(3, group, &[]);
                pass.draw_indirect(arguments, 0);
            }
        }
    }
}

/// Identity depth is independent of WBOIT so transparent beauty never occludes later layers.
pub(crate) fn copy_depth<D: Device>(ctx: &mut PassContext<'_, D>) {
    if ctx.scene.segmentation_draws().next().is_none() {
        return;
    }
    if let (Some(source), Some(target)) = (
        ctx.resources.texture(DEPTH_RESOURCE),
        ctx.resources.texture(crate::passes::SEGMENT_DEPTH_RESOURCE),
    ) {
        ctx.encoder
            .copy_texture_to_texture(source, target, ctx.resources.extent());
    }
}
