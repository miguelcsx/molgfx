//! Pass state: pipelines and bind groups created at load, recorded per
//! frame through plain function pointers.

mod cull;
mod deferred;
mod draw;
mod dynamics;
mod pipelines;
mod postprocess;
mod screen;
mod state;
mod surfaces;
mod transparency;

use deferred::{clear, lighting, opaque_depth, shadow};
use draw::{bond, cartoon, helix_fit, point, primitive, sphere};
use dynamics::{occupancy, particle_motion, trajectory};
use pipelines::{
    ligand_pose_pipelines, primitive_pipelines, primitive_shadow_pipelines, relation_resolve,
    visual_pipelines,
};
use postprocess::{
    ambient_occlusion, ao_denoise, bloom, depth_of_field, motion_blur, temporal, tonemap,
};
use screen::{label, overlay};
pub(crate) use state::build;
use state::{bindings, lazy, registry, resources};
use surfaces::{surface, surface_components, surface_field};
use transparency::{interaction, oit, oit_composite};

pub(crate) use ambient_occlusion::AmbientOcclusionPass;
pub(crate) use ao_denoise::AoDenoisePass;
pub(crate) use bindings::FrameBindings;
pub(crate) use bloom::BloomPass;
pub(crate) use bond::BondPass;
pub(crate) use cartoon::CartoonPass;
pub(crate) use clear::ClearPass;
pub(crate) use cull::{CullLayouts, CullPass};
pub(crate) use depth_of_field::DepthOfFieldPass;
pub(crate) use helix_fit::HelixFitPass;
pub(crate) use interaction::InteractionPass;
pub(crate) use label::LabelPass;
pub(crate) use lazy::Lazy;
pub(crate) use lighting::LightingPass;
pub(crate) use motion_blur::MotionBlurPass;
pub(crate) use occupancy::{OccupancyBoundsFormat, OccupancyPass};
pub(crate) use oit::OitPass;
pub(crate) use oit::segmentation::record as record_segmentations;
pub(crate) use oit_composite::OitCompositePass;
pub(crate) use opaque_depth::OpaqueDepthPass;
pub(crate) use overlay::OverlayPass;
pub(crate) use particle_motion::ParticleMotionPass;
pub(crate) use point::PointPass;
pub(crate) use primitive::PrimitivePass;
pub(crate) use registry::PassRegistry;
pub(crate) use relation_resolve::RelationResolvePass;
pub(crate) use resources::{
    ALBEDO_RESOURCE, AO_DENOISED_RESOURCE, AO_FORMAT, AO_RESOURCE, BLOOM_A_RESOURCE,
    BLOOM_B_RESOURCE, BLOOM_C_RESOURCE, COMPOSITE_RESOURCE, DOF_RESOURCE, DOF_TILE_RESOURCE,
    ENTITY_RESOURCE, GBUFFER_ALBEDO_FORMAT, GBUFFER_DEPTH, GBUFFER_MOTION_FORMAT,
    GBUFFER_NORMAL_FORMAT, HDR_RESOURCE, HISTORY_A_RESOURCE, HISTORY_B_RESOURCE,
    MOTION_BLUR_RESOURCE, MOTION_RESOURCE, NORMAL_RESOURCE, OIT_ACCUM_RESOURCE,
    OIT_REVEAL_RESOURCE, OPAQUE_DEPTH_RESOURCE, SEGMENT_LABEL_RESOURCE, SEGMENT_VOLUME_RESOURCE,
    SHADOW_RESOURCE, STRUCTURE_RESOURCE, begin_gbuffer_pass, fullscreen_pipeline, gbuffer_pipeline,
    gbuffer_targets, replace_target, segmentation_targets,
};
pub(crate) use shadow::ShadowPass;
pub(crate) use sphere::{DEPTH_RESOURCE, SpherePass};
pub(crate) use surface::SurfacePass;
pub(crate) use surface_components::SurfaceComponentPass;
pub(crate) use surface_field::SurfaceFieldPass;
pub(crate) use temporal::TemporalPass;
pub(crate) use tonemap::TonemapPass;
pub(crate) use trajectory::TrajectoryPass;

pub(crate) use oit::segmentation::{
    copy_depth as copy_segment_depth, record_identity as record_segment_identity,
};
pub(crate) use resources::SEGMENT_DEPTH_RESOURCE;
