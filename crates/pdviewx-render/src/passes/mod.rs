//! Pass state: pipelines and bind groups created at load, recorded per
//! frame through plain function pointers.

mod ambient_occlusion;
mod ao_denoise;
mod bindings;
mod bloom;
mod bond;
mod cartoon;
mod clear;
mod cull;
mod depth_of_field;
mod interaction;
mod label;
mod lighting;
mod motion_blur;
mod oit;
mod oit_composite;
mod overlay;
mod particle_motion;
mod point;
mod primitive;
mod registry;
mod resources;
mod shadow;
mod sphere;
mod surface;
mod surface_field;
mod temporal;
mod tonemap;
mod trajectory;

pub(crate) use ambient_occlusion::AmbientOcclusionPass;
pub(crate) use ao_denoise::AoDenoisePass;
pub(crate) use bindings::FrameBindings;
pub(crate) use bloom::BloomPass;
pub(crate) use bond::BondPass;
pub(crate) use cartoon::CartoonPass;
pub(crate) use clear::ClearPass;
pub(crate) use cull::CullPass;
pub(crate) use depth_of_field::DepthOfFieldPass;
pub(crate) use interaction::InteractionPass;
pub(crate) use label::LabelPass;
pub(crate) use lighting::LightingPass;
pub(crate) use motion_blur::MotionBlurPass;
pub(crate) use oit::OitPass;
pub(crate) use oit_composite::OitCompositePass;
pub(crate) use overlay::OverlayPass;
pub(crate) use particle_motion::ParticleMotionPass;
pub(crate) use point::PointPass;
pub(crate) use primitive::PrimitivePass;
pub(crate) use registry::PassRegistry;
pub(crate) use resources::{
    ALBEDO_RESOURCE, AO_DENOISED_RESOURCE, AO_FORMAT, AO_RESOURCE, BLOOM_A_RESOURCE,
    BLOOM_B_RESOURCE, BLOOM_C_RESOURCE, COMPOSITE_RESOURCE, DOF_RESOURCE, DOF_TILE_RESOURCE,
    ENTITY_RESOURCE, GBUFFER_ALBEDO_FORMAT, GBUFFER_MOTION_FORMAT, GBUFFER_NORMAL_FORMAT,
    HDR_RESOURCE, HISTORY_A_RESOURCE, HISTORY_B_RESOURCE, MOTION_BLUR_RESOURCE, MOTION_RESOURCE,
    NORMAL_RESOURCE, OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE, SEGMENT_LABEL_RESOURCE,
    SEGMENT_VOLUME_RESOURCE, SHADOW_RESOURCE, STRUCTURE_RESOURCE, gbuffer_targets,
    segmentation_targets,
};
pub(crate) use shadow::ShadowPass;
pub(crate) use sphere::{DEPTH_RESOURCE, SpherePass};
pub(crate) use surface::SurfacePass;
pub(crate) use surface_field::SurfaceFieldPass;
pub(crate) use temporal::TemporalPass;
pub(crate) use tonemap::TonemapPass;
pub(crate) use trajectory::TrajectoryPass;
