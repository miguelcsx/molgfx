//! Render-graph declaration for weighted transparency and annotations.

use crate::graph::{PassKind, PassNode};
use crate::passes::{
    AO_RESOURCE, COMPOSITE_RESOURCE, ClearPass, DEPTH_RESOURCE, ENTITY_RESOURCE, HDR_RESOURCE,
    InteractionPass, LabelPass, OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE, OPAQUE_DEPTH_RESOURCE,
    OitCompositePass, OitPass, OpaqueDepthPass, SEGMENT_LABEL_RESOURCE, SEGMENT_VOLUME_RESOURCE,
    STRUCTURE_RESOURCE,
};
use molgfx_gpu::Device;
use smallvec::smallvec;

/// What a transparent pass reads from the opaque scene: its occlusion, its depth
/// and, where the live depth cannot be bound beside the depth test, the copy.
fn opaque_scene_reads(depth_snapshot: bool) -> smallvec::SmallVec<[crate::graph::ResourceId; 4]> {
    let mut reads = smallvec![DEPTH_RESOURCE, AO_RESOURCE];
    if depth_snapshot {
        reads.push(OPAQUE_DEPTH_RESOURCE);
    }
    reads
}

pub(super) fn transparency_nodes<D: Device>(depth_snapshot: bool) -> Vec<PassNode<D>> {
    let mut nodes = Vec::with_capacity(14);
    nodes.extend(transparency_clear_nodes());
    if depth_snapshot {
        nodes.push(PassNode {
            name: "opaque depth snapshot",
            reads: smallvec![DEPTH_RESOURCE],
            writes: smallvec![OPAQUE_DEPTH_RESOURCE],
            kind: PassKind::Graphics,
            record: OpaqueDepthPass::record,
        });
    }
    nodes.extend(transparency_molecule_nodes(depth_snapshot));
    nodes.extend(transparency_volume_nodes(depth_snapshot));
    nodes.extend(transparency_annotation_nodes());
    nodes.push(transparency_composite_node());
    nodes
}

fn transparency_clear_nodes<D: Device>() -> [PassNode<D>; 2] {
    [
        PassNode {
            name: "clear categorical segment ids",
            reads: smallvec![],
            writes: smallvec![SEGMENT_VOLUME_RESOURCE, SEGMENT_LABEL_RESOURCE],
            kind: PassKind::Graphics,
            record: ClearPass::segment_ids,
        },
        PassNode {
            name: "clear transparency accumulation",
            reads: smallvec![],
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::clear,
        },
    ]
}

fn transparency_molecule_nodes<D: Device>(depth_snapshot: bool) -> [PassNode<D>; 6] {
    [
        PassNode {
            name: "transparent sphere impostors",
            reads: opaque_scene_reads(depth_snapshot),
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::spheres,
        },
        PassNode {
            name: "transparent atom points",
            reads: opaque_scene_reads(depth_snapshot),
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::points,
        },
        PassNode {
            name: "transparent bond capsules",
            reads: opaque_scene_reads(depth_snapshot),
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::bonds,
        },
        PassNode {
            name: "transparent cartoon ribbons",
            reads: opaque_scene_reads(depth_snapshot),
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::cartoons,
        },
        PassNode {
            name: "transparent implicit molecular surfaces",
            reads: opaque_scene_reads(depth_snapshot),
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::surfaces,
        },
        PassNode {
            name: "transparent analytic primitives",
            reads: opaque_scene_reads(depth_snapshot),
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::primitive,
        },
    ]
}

fn transparency_volume_nodes<D: Device>(depth_snapshot: bool) -> [PassNode<D>; 2] {
    [
        PassNode {
            name: "direct density volumes",
            reads: opaque_scene_reads(depth_snapshot),
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::volumes,
        },
        PassNode {
            name: "categorical segment volumes",
            reads: opaque_scene_reads(depth_snapshot),
            writes: smallvec![
                OIT_ACCUM_RESOURCE,
                OIT_REVEAL_RESOURCE,
                SEGMENT_VOLUME_RESOURCE,
                SEGMENT_LABEL_RESOURCE
            ],
            kind: PassKind::Graphics,
            record: OitPass::segmentations,
        },
    ]
}

fn transparency_annotation_nodes<D: Device>() -> [PassNode<D>; 3] {
    [
        PassNode {
            name: "molecular interaction glyphs",
            reads: smallvec![DEPTH_RESOURCE],
            writes: smallvec![
                OIT_ACCUM_RESOURCE,
                OIT_REVEAL_RESOURCE,
                ENTITY_RESOURCE,
                STRUCTURE_RESOURCE
            ],
            kind: PassKind::Graphics,
            record: InteractionPass::record,
        },
        PassNode {
            name: "semantic label decluttering",
            reads: smallvec![],
            writes: smallvec![],
            kind: PassKind::Compute,
            record: LabelPass::declutter,
        },
        PassNode {
            name: "semantic labels and measurements",
            reads: smallvec![DEPTH_RESOURCE],
            writes: smallvec![
                OIT_ACCUM_RESOURCE,
                OIT_REVEAL_RESOURCE,
                ENTITY_RESOURCE,
                STRUCTURE_RESOURCE
            ],
            kind: PassKind::Graphics,
            record: LabelPass::render,
        },
    ]
}

fn transparency_composite_node<D: Device>() -> PassNode<D> {
    PassNode {
        name: "weighted transparency composite",
        reads: smallvec![HDR_RESOURCE, OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
        writes: smallvec![COMPOSITE_RESOURCE],
        kind: PassKind::Graphics,
        record: OitCompositePass::record,
    }
}
