//! The presentation tail of the frame graph: temporal resolve, depth of
//! field, motion blur, bloom, tonemap and screen overlays.

use crate::graph::{self, PassKind, PassNode, SampleCadence};
use crate::passes::{
    BLOOM_A_RESOURCE, BLOOM_B_RESOURCE, BLOOM_C_RESOURCE, BloomPass, COMPOSITE_RESOURCE,
    DEPTH_RESOURCE, DOF_RESOURCE, DOF_TILE_RESOURCE, DepthOfFieldPass, HDR_RESOURCE,
    HISTORY_A_RESOURCE, HISTORY_B_RESOURCE, MOTION_BLUR_RESOURCE, MOTION_RESOURCE, MotionBlurPass,
    OverlayPass, TemporalPass, TonemapPass,
};
use molgfx_gpu::Device;
use smallvec::smallvec;

pub(super) fn presentation_nodes<D: Device>(
    depth_of_field: bool,
    bloom: bool,
    motion_blur: bool,
) -> Vec<PassNode<D>> {
    let mut nodes = vec![PassNode {
        name: "temporal HDR resolve",
        reads: smallvec![
            HDR_RESOURCE,
            COMPOSITE_RESOURCE,
            DEPTH_RESOURCE,
            HISTORY_A_RESOURCE,
            HISTORY_B_RESOURCE,
            MOTION_RESOURCE
        ],
        writes: smallvec![HISTORY_A_RESOURCE, HISTORY_B_RESOURCE],
        kind: PassKind::Graphics,
        record: TemporalPass::record,
        cadence: SampleCadence::Every,
    }];
    if depth_of_field {
        nodes.extend([
            PassNode {
                name: "depth-of-field tile classification",
                reads: smallvec![DEPTH_RESOURCE],
                writes: smallvec![DOF_TILE_RESOURCE],
                kind: PassKind::Graphics,
                record: DepthOfFieldPass::classify,
                cadence: SampleCadence::Last,
            },
            PassNode {
                name: "depth-of-field bounded gather",
                reads: smallvec![
                    HISTORY_A_RESOURCE,
                    HISTORY_B_RESOURCE,
                    DEPTH_RESOURCE,
                    DOF_TILE_RESOURCE
                ],
                writes: smallvec![DOF_RESOURCE],
                kind: PassKind::Graphics,
                record: DepthOfFieldPass::resolve,
                cadence: SampleCadence::Last,
            },
        ]);
    }
    if motion_blur {
        nodes.push(motion_blur_node(depth_of_field));
    }
    if bloom {
        nodes.extend(bloom_nodes(depth_of_field, motion_blur));
    }
    nodes.push(tonemap_node(depth_of_field, motion_blur, bloom));
    nodes.push(PassNode {
        name: "screen overlays",
        reads: smallvec![],
        writes: smallvec![graph::ResourceId::SWAPCHAIN],
        kind: PassKind::Graphics,
        record: OverlayPass::record,
        cadence: SampleCadence::Last,
    });
    nodes
}

fn motion_blur_node<D: Device>(depth_of_field: bool) -> PassNode<D> {
    PassNode {
        name: "camera-shutter motion blur",
        reads: {
            let mut reads = presentation_input(depth_of_field, false);
            reads.push(MOTION_RESOURCE);
            reads
        },
        writes: smallvec![MOTION_BLUR_RESOURCE],
        kind: PassKind::Graphics,
        record: MotionBlurPass::record,
        cadence: SampleCadence::Last,
    }
}

fn bloom_nodes<D: Device>(depth_of_field: bool, motion_blur: bool) -> [PassNode<D>; 3] {
    [
        PassNode {
            name: "bloom bright pass",
            reads: presentation_input(depth_of_field, motion_blur),
            writes: smallvec![BLOOM_A_RESOURCE],
            kind: PassKind::Graphics,
            record: BloomPass::bright,
            cadence: SampleCadence::Last,
        },
        PassNode {
            name: "bloom horizontal blur",
            reads: smallvec![BLOOM_A_RESOURCE],
            writes: smallvec![BLOOM_B_RESOURCE],
            kind: PassKind::Graphics,
            record: BloomPass::horizontal,
            cadence: SampleCadence::Last,
        },
        PassNode {
            name: "bloom vertical blur",
            reads: smallvec![BLOOM_B_RESOURCE],
            writes: smallvec![BLOOM_C_RESOURCE],
            kind: PassKind::Graphics,
            record: BloomPass::vertical,
            cadence: SampleCadence::Last,
        },
    ]
}

fn tonemap_node<D: Device>(depth_of_field: bool, motion_blur: bool, bloom: bool) -> PassNode<D> {
    let mut reads = presentation_input(depth_of_field, motion_blur);
    if bloom {
        reads.push(BLOOM_C_RESOURCE);
    }
    PassNode {
        name: "HDR tonemap",
        reads,
        writes: smallvec![graph::ResourceId::SWAPCHAIN],
        kind: PassKind::Graphics,
        record: TonemapPass::record,
        cadence: SampleCadence::Last,
    }
}

fn presentation_input(
    depth_of_field: bool,
    motion_blur: bool,
) -> smallvec::SmallVec<[graph::ResourceId; 4]> {
    if motion_blur {
        smallvec![MOTION_BLUR_RESOURCE]
    } else if depth_of_field {
        smallvec![DOF_RESOURCE]
    } else {
        smallvec![HISTORY_A_RESOURCE, HISTORY_B_RESOURCE]
    }
}
