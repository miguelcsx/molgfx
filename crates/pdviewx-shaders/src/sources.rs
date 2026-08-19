//! The composed, validated WGSL sources, embedded as strings.
//!
//! Each constant is a standalone standard-WGSL unit produced by the build
//! script: includes resolved, validated by naga before it could compile
//! into this crate.

/// Screen-space molecular ambient occlusion.
pub const AMBIENT_OCCLUSION: &str =
    include_str!(concat!(env!("OUT_DIR"), "/ambient_occlusion.wgsl"));

/// Progressive shared-BVH cavity occlusion and area-light shadows.
pub const QUALITY_AO: &str = include_str!(concat!(env!("OUT_DIR"), "/quality_ao.wgsl"));

/// Gbuffer sphere impostors.
pub const GEOMETRY_SPHERE: &str = include_str!(concat!(env!("OUT_DIR"), "/sphere.wgsl"));

/// Gbuffer analytic bond capsules.
pub const GEOMETRY_BOND: &str = include_str!(concat!(env!("OUT_DIR"), "/bond.wgsl"));

/// Transport-frame cartoon geometry.
pub const GEOMETRY_CARTOON: &str = include_str!(concat!(env!("OUT_DIR"), "/cartoon.wgsl"));

/// BVH-bounded implicit molecular surfaces.
pub const GEOMETRY_SURFACE: &str = include_str!(concat!(env!("OUT_DIR"), "/surface.wgsl"));

/// Analytic caller-authored ellipsoids, carbohydrate symbols and planes.
pub const GEOMETRY_PRIMITIVE: &str = include_str!(concat!(env!("OUT_DIR"), "/primitive.wgsl"));

/// Scene-fit analytic primitive shadow map.
pub const SHADOW: &str = include_str!(concat!(env!("OUT_DIR"), "/shadow.wgsl"));

/// Depth-only cartoon-ribbon shadow caster, pulled from ribbon storage.
pub const SHADOW_RIBBON: &str = include_str!(concat!(env!("OUT_DIR"), "/shadow_ribbon.wgsl"));

/// Pixel-stable circular atom points.
pub const GEOMETRY_POINT: &str = include_str!(concat!(env!("OUT_DIR"), "/point.wgsl"));

/// Pixel-stable molecular interaction glyphs.
pub const GEOMETRY_INTERACTION: &str = include_str!(concat!(env!("OUT_DIR"), "/interaction.wgsl"));

/// Analytic stroke-SDF labels, markers and measurement guides.
pub const GEOMETRY_LABEL: &str = include_str!(concat!(env!("OUT_DIR"), "/label.wgsl"));

/// Post-tonemap depth-independent screen overlays.
pub const OVERLAY: &str = include_str!(concat!(env!("OUT_DIR"), "/overlay.wgsl"));

/// Deterministic GPU label decluttering and indirect compaction.
pub const LABEL_DECLUTTER: &str = include_str!(concat!(env!("OUT_DIR"), "/label_declutter.wgsl"));

/// Direct ray-marched caller scalar volumes.
pub const VOLUME: &str = include_str!(concat!(env!("OUT_DIR"), "/scalar.wgsl"));

/// Ray-marched caller-supplied categorical segmentation volumes.
pub const SEGMENTATION: &str = include_str!(concat!(env!("OUT_DIR"), "/segmentation.wgsl"));

/// Persistent rolling-probe SES field generation.
pub const SURFACE_FIELD_COMPUTE: &str =
    include_str!(concat!(env!("OUT_DIR"), "/surface_field_compute.wgsl"));

/// Rolling-probe erosion over the persistent inflated field.
pub const SURFACE_FIELD_ERODE: &str =
    include_str!(concat!(env!("OUT_DIR"), "/surface_field_erode.wgsl"));

/// Visibility compaction and indirect argument generation.
pub const CULL: &str = include_str!(concat!(env!("OUT_DIR"), "/cull.wgsl"));

/// Topology-stable two-frame coordinate interpolation.
pub const TRAJECTORY: &str = include_str!(concat!(env!("OUT_DIR"), "/trajectory.wgsl"));

/// Fixed-step caller-supplied visual particle advection.
pub const PARTICLE_ADVECTION: &str =
    include_str!(concat!(env!("OUT_DIR"), "/particle_advection.wgsl"));

/// HDR deferred molecular lighting.
pub const LIGHTING: &str = include_str!(concat!(env!("OUT_DIR"), "/deferred.wgsl"));

/// Temporal HDR history resolve.
pub const TEMPORAL_RESOLVE: &str = include_str!(concat!(env!("OUT_DIR"), "/temporal_resolve.wgsl"));

/// Tile-classified thin-lens depth of field.
pub const DEPTH_OF_FIELD: &str = include_str!(concat!(env!("OUT_DIR"), "/depth_of_field.wgsl"));

/// Weighted-blended transparency composite.
pub const OIT_COMPOSITE: &str = include_str!(concat!(env!("OUT_DIR"), "/oit_composite.wgsl"));

/// Edge-aware occlusion denoise.
pub const AO_DENOISE: &str = include_str!(concat!(env!("OUT_DIR"), "/ao_denoise.wgsl"));

/// Bright-pass downsample and separable bloom blur.
pub const BLOOM: &str = include_str!(concat!(env!("OUT_DIR"), "/bloom.wgsl"));

/// HDR tonemapping and presentation.
pub const TONEMAP: &str = include_str!(concat!(env!("OUT_DIR"), "/tonemap.wgsl"));

/// Bounded camera-shutter gather driven by true surface motion.
pub const MOTION_BLUR: &str = include_str!(concat!(env!("OUT_DIR"), "/motion_blur.wgsl"));
