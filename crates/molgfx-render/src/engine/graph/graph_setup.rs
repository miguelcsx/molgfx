//! Render-graph declaration: the pass nodes and the transient resources they
//! read and write.
//!
//! Kept beside engine construction rather than inside it: the graph's shape is
//! its own concern, and it changes for different reasons than device opening
//! and pipeline creation do.

use crate::engine::profile::ResolvedRenderPlan;
use crate::graph::{self, PassKind, PassNode, ResourceDesc, SizeClass};
use crate::passes::{
    ALBEDO_RESOURCE, AO_DENOISED_RESOURCE, AO_RESOURCE, AmbientOcclusionPass, AoDenoisePass,
    BLOOM_A_RESOURCE, BLOOM_B_RESOURCE, BLOOM_C_RESOURCE, BloomPass, BondPass, COMPOSITE_RESOURCE,
    CartoonPass, ClearPass, CullPass, DEPTH_RESOURCE, DOF_RESOURCE, DOF_TILE_RESOURCE,
    DepthOfFieldPass, ENTITY_RESOURCE, HDR_RESOURCE, HISTORY_A_RESOURCE, HISTORY_B_RESOURCE,
    LightingPass, MOTION_BLUR_RESOURCE, MOTION_RESOURCE, MotionBlurPass, NORMAL_RESOURCE,
    OverlayPass, PointPass, PrimitivePass, SHADOW_RESOURCE, STRUCTURE_RESOURCE, ShadowPass,
    SpherePass, SurfacePass, TemporalPass, TonemapPass,
};
use molgfx_gpu::{Device, TextureFormat, TextureUsage};
use smallvec::smallvec;

#[cfg(test)]
#[path = "graph_setup_tests.rs"]
mod tests;

/// The optional passes a frame graph includes.
///
/// Every field changes the graph's shape, so a change to any of them rebuilds
/// the schedule and the transient pool.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(in crate::engine) struct GraphTopology {
    pub(in crate::engine) depth_of_field: bool,
    pub(in crate::engine) bloom: bool,
    pub(in crate::engine) motion_blur: bool,
    pub(in crate::engine) transparent_depth: TransparentDepth,
}

/// What transparent passes bind as the opaque scene's depth.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(in crate::engine) enum TransparentDepth {
    /// The live depth, which the passes also test against.
    #[default]
    Live,
    /// A copy, because the device cannot bind the live depth beside testing
    /// against it.
    Copy,
}

impl GraphTopology {
    /// The topology a plan needs on a device.
    pub(in crate::engine) fn of<D: Device>(plan: &ResolvedRenderPlan, device: &D) -> Self {
        Self {
            depth_of_field: plan.depth_of_field().is_some(),
            bloom: plan.bloom().is_some(),
            motion_blur: plan.motion_blur().is_some(),
            transparent_depth: if device.capabilities().depth_read_while_sampled() {
                TransparentDepth::Live
            } else {
                TransparentDepth::Copy
            },
        }
    }
}

pub(in crate::engine) fn realtime_nodes<D: Device>(topology: GraphTopology) -> Vec<PassNode<D>> {
    let depth_snapshot = topology.transparent_depth == TransparentDepth::Copy;
    let gbuffer = smallvec![
        ALBEDO_RESOURCE,
        NORMAL_RESOURCE,
        ENTITY_RESOURCE,
        STRUCTURE_RESOURCE,
        MOTION_RESOURCE,
        DEPTH_RESOURCE
    ];
    let mut nodes = vec![
        PassNode {
            name: "clear gbuffer",
            reads: smallvec![],
            writes: gbuffer.clone(),
            kind: PassKind::Graphics,
            record: ClearPass::record,
            final_sample_only: false,
        },
        PassNode {
            name: "visibility culling",
            reads: smallvec![],
            writes: smallvec![],
            kind: PassKind::Compute,
            record: CullPass::record,
            final_sample_only: false,
        },
        PassNode {
            name: "scene-fit analytic shadows",
            reads: smallvec![],
            writes: smallvec![SHADOW_RESOURCE],
            kind: PassKind::Graphics,
            record: ShadowPass::record,
            final_sample_only: false,
        },
        PassNode {
            name: "sphere impostors",
            reads: smallvec![],
            writes: gbuffer.clone(),
            kind: PassKind::Graphics,
            record: SpherePass::record,
            final_sample_only: false,
        },
        point_node(gbuffer.clone()),
        PassNode {
            name: "bond capsules",
            reads: smallvec![],
            writes: gbuffer.clone(),
            kind: PassKind::Graphics,
            record: BondPass::record,
            final_sample_only: false,
        },
        PassNode {
            name: "cartoon ribbons",
            reads: smallvec![],
            writes: gbuffer.clone(),
            kind: PassKind::Graphics,
            record: CartoonPass::record,
            final_sample_only: false,
        },
        PassNode {
            name: "analytic primitives",
            reads: smallvec![],
            writes: gbuffer.clone(),
            kind: PassKind::Graphics,
            record: PrimitivePass::record,
            final_sample_only: false,
        },
        PassNode {
            name: "implicit molecular surfaces",
            reads: smallvec![],
            writes: gbuffer,
            kind: PassKind::Graphics,
            record: SurfacePass::record,
            final_sample_only: false,
        },
    ];
    nodes.extend(lighting_nodes());
    nodes.extend(crate::engine::graph_transparency::transparency_nodes(
        depth_snapshot,
    ));
    nodes.extend(presentation_nodes(
        topology.depth_of_field,
        topology.bloom,
        topology.motion_blur,
    ));
    nodes
}

/// Ambient occlusion, its denoise and the deferred lighting of the opaque scene.
fn lighting_nodes<D: Device>() -> [PassNode<D>; 3] {
    [
        PassNode {
            name: "molecular ambient occlusion",
            reads: smallvec![DEPTH_RESOURCE, NORMAL_RESOURCE],
            writes: smallvec![AO_RESOURCE],
            kind: PassKind::Graphics,
            record: AmbientOcclusionPass::record,
            final_sample_only: false,
        },
        PassNode {
            name: "occlusion denoise",
            reads: smallvec![AO_RESOURCE, DEPTH_RESOURCE, NORMAL_RESOURCE],
            writes: smallvec![AO_DENOISED_RESOURCE],
            kind: PassKind::Graphics,
            record: AoDenoisePass::record,
            final_sample_only: false,
        },
        PassNode {
            name: "HDR deferred lighting",
            reads: smallvec![
                ALBEDO_RESOURCE,
                NORMAL_RESOURCE,
                DEPTH_RESOURCE,
                AO_DENOISED_RESOURCE,
                SHADOW_RESOURCE
            ],
            writes: smallvec![HDR_RESOURCE],
            kind: PassKind::Graphics,
            record: LightingPass::record,
            final_sample_only: false,
        },
    ]
}

fn presentation_nodes<D: Device>(
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
        final_sample_only: false,
    }];
    if depth_of_field {
        nodes.extend([
            PassNode {
                name: "depth-of-field tile classification",
                reads: smallvec![DEPTH_RESOURCE],
                writes: smallvec![DOF_TILE_RESOURCE],
                kind: PassKind::Graphics,
                record: DepthOfFieldPass::classify,
                final_sample_only: true,
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
                final_sample_only: true,
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
        final_sample_only: true,
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
        final_sample_only: true,
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
            final_sample_only: true,
        },
        PassNode {
            name: "bloom horizontal blur",
            reads: smallvec![BLOOM_A_RESOURCE],
            writes: smallvec![BLOOM_B_RESOURCE],
            kind: PassKind::Graphics,
            record: BloomPass::horizontal,
            final_sample_only: true,
        },
        PassNode {
            name: "bloom vertical blur",
            reads: smallvec![BLOOM_B_RESOURCE],
            writes: smallvec![BLOOM_C_RESOURCE],
            kind: PassKind::Graphics,
            record: BloomPass::vertical,
            final_sample_only: true,
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
        final_sample_only: true,
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

fn point_node<D: Device>(gbuffer: smallvec::SmallVec<[graph::ResourceId; 4]>) -> PassNode<D> {
    PassNode {
        name: "atom points",
        reads: smallvec![],
        writes: gbuffer,
        kind: PassKind::Graphics,
        record: PointPass::record,
        final_sample_only: false,
    }
}

pub(in crate::engine) fn realtime_resources() -> Vec<ResourceDesc> {
    let sampled_target = TextureUsage::RENDER_ATTACHMENT.union(TextureUsage::TEXTURE_BINDING);
    let readback_target = sampled_target.union(TextureUsage::COPY_SRC);
    let mut resources = vec![
        // Copyable so a device that cannot sample the live depth beside testing
        // against it can hand transparent passes a copy.
        resource(
            "frame depth",
            TextureFormat::Depth32Float,
            sampled_target.union(TextureUsage::COPY_SRC),
        ),
        resource(
            "gbuffer albedo material",
            crate::passes::GBUFFER_ALBEDO_FORMAT,
            sampled_target,
        ),
        resource(
            "gbuffer normal roughness",
            crate::passes::GBUFFER_NORMAL_FORMAT,
            sampled_target,
        ),
        resource(
            "gbuffer entity id",
            TextureFormat::R32Uint,
            TextureUsage::RENDER_ATTACHMENT.union(TextureUsage::COPY_SRC),
        ),
        resource(
            "gbuffer structure id",
            TextureFormat::R32Uint,
            TextureUsage::RENDER_ATTACHMENT.union(TextureUsage::COPY_SRC),
        ),
        resource(
            "ambient visibility and contact shadow",
            TextureFormat::Rgba8Unorm,
            sampled_target,
        ),
        resource("HDR lighting", TextureFormat::Rgba16Float, readback_target),
        persistent_resource(
            "temporal history A",
            TextureFormat::Rgba16Float,
            readback_target,
        ),
        persistent_resource(
            "temporal history B",
            TextureFormat::Rgba16Float,
            readback_target,
        ),
        resource(
            "transparent color accumulation",
            TextureFormat::Rgba16Float,
            readback_target,
        ),
        resource(
            "transparent revealage",
            TextureFormat::R8Unorm,
            sampled_target,
        ),
        resource(
            "composited HDR",
            TextureFormat::Rgba16Float,
            readback_target,
        ),
        ResourceDesc {
            label: "depth-of-field tile classification",
            format: TextureFormat::R8Unorm,
            size: SizeClass::Tiles16,
            usage: sampled_target,
            persistent: false,
        },
        resource(
            "depth-of-field resolved HDR",
            TextureFormat::Rgba16Float,
            readback_target,
        ),
        resource(
            "opaque screen-space motion",
            crate::passes::GBUFFER_MOTION_FORMAT,
            sampled_target,
        ),
    ];
    resources.extend(presentation_resources(sampled_target));
    resources
}

fn presentation_resources(sampled_target: TextureUsage) -> [ResourceDesc; 10] {
    [
        ResourceDesc {
            label: "bloom ping",
            format: TextureFormat::Rgba16Float,
            size: SizeClass::Quarter,
            usage: sampled_target,
            persistent: false,
        },
        ResourceDesc {
            label: "bloom pong",
            format: TextureFormat::Rgba16Float,
            size: SizeClass::Quarter,
            usage: sampled_target,
            persistent: false,
        },
        ResourceDesc {
            label: "bloom resolved",
            format: TextureFormat::Rgba16Float,
            size: SizeClass::Quarter,
            usage: sampled_target,
            persistent: false,
        },
        // Declaration order is the resource identity: every `ResourceId` is an
        // index into this list, so new entries append and never insert.
        resource(
            "denoised ambient visibility and contact shadow",
            TextureFormat::Rgba8Unorm,
            sampled_target,
        ),
        resource(
            "categorical segment source id",
            TextureFormat::R32Uint,
            TextureUsage::RENDER_ATTACHMENT.union(TextureUsage::COPY_SRC),
        ),
        resource(
            "categorical segment label",
            TextureFormat::R32Uint,
            TextureUsage::RENDER_ATTACHMENT.union(TextureUsage::COPY_SRC),
        ),
        ResourceDesc {
            label: "scene-fit shadow map",
            format: TextureFormat::Depth32Float,
            size: SizeClass::Shadow,
            usage: TextureUsage::RENDER_ATTACHMENT.union(TextureUsage::TEXTURE_BINDING),
            persistent: false,
        },
        resource(
            "camera-shutter motion-blurred HDR",
            TextureFormat::Rgba16Float,
            sampled_target.union(TextureUsage::COPY_SRC),
        ),
        resource(
            "opaque depth snapshot",
            TextureFormat::Depth32Float,
            TextureUsage::TEXTURE_BINDING.union(TextureUsage::COPY_DST),
        ),
        resource(
            "categorical identity depth",
            TextureFormat::Depth32Float,
            TextureUsage::RENDER_ATTACHMENT.union(TextureUsage::COPY_DST),
        ),
    ]
}

const fn resource(label: &'static str, format: TextureFormat, usage: TextureUsage) -> ResourceDesc {
    ResourceDesc {
        label,
        format,
        size: SizeClass::Full,
        usage,
        persistent: false,
    }
}

const fn persistent_resource(
    label: &'static str,
    format: TextureFormat,
    usage: TextureUsage,
) -> ResourceDesc {
    ResourceDesc {
        label,
        format,
        size: SizeClass::Full,
        usage,
        persistent: true,
    }
}
