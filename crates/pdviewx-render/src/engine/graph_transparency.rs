//! Render-graph declaration for weighted transparency and annotations.

use crate::graph::{PassKind, PassNode};
use crate::passes::{
    AO_RESOURCE, COMPOSITE_RESOURCE, ClearPass, DEPTH_RESOURCE, ENTITY_RESOURCE, HDR_RESOURCE,
    InteractionPass, LabelPass, OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE, OitCompositePass, OitPass,
    SEGMENT_LABEL_RESOURCE, SEGMENT_VOLUME_RESOURCE, STRUCTURE_RESOURCE,
};
use pdviewx_gpu::Device;
use smallvec::smallvec;

pub(super) fn transparency_nodes<D: Device>() -> Vec<PassNode<D>> {
    let mut nodes = Vec::with_capacity(13);
    nodes.extend(transparency_clear_nodes());
    nodes.extend(transparency_molecule_nodes());
    nodes.extend(transparency_volume_nodes());
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

fn transparency_molecule_nodes<D: Device>() -> [PassNode<D>; 6] {
    [
        PassNode {
            name: "transparent sphere impostors",
            reads: smallvec![DEPTH_RESOURCE, AO_RESOURCE],
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::spheres,
        },
        PassNode {
            name: "transparent atom points",
            reads: smallvec![DEPTH_RESOURCE, AO_RESOURCE],
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::points,
        },
        PassNode {
            name: "transparent bond capsules",
            reads: smallvec![DEPTH_RESOURCE, AO_RESOURCE],
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::bonds,
        },
        PassNode {
            name: "transparent cartoon ribbons",
            reads: smallvec![DEPTH_RESOURCE, AO_RESOURCE],
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::cartoons,
        },
        PassNode {
            name: "transparent implicit molecular surfaces",
            reads: smallvec![DEPTH_RESOURCE, AO_RESOURCE],
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::surfaces,
        },
        PassNode {
            name: "transparent analytic primitives",
            reads: smallvec![DEPTH_RESOURCE, AO_RESOURCE],
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::primitive,
        },
    ]
}

fn transparency_volume_nodes<D: Device>() -> [PassNode<D>; 2] {
    [
        PassNode {
            name: "direct density volumes",
            reads: smallvec![DEPTH_RESOURCE, AO_RESOURCE],
            writes: smallvec![OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE],
            kind: PassKind::Graphics,
            record: OitPass::volumes,
        },
        PassNode {
            name: "categorical segment volumes",
            reads: smallvec![DEPTH_RESOURCE, AO_RESOURCE],
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
