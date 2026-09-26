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

/// [`QUALITY_AO`] with the visual resolver replaced by generated code.
pub const QUALITY_AO_SPECIALIZED: &str =
    include_str!(concat!(env!("OUT_DIR"), "/quality_ao.specialized.wgsl"));

/// Progressive analytic AO/shadows using hardware ray queries for traversal.
pub const QUALITY_AO_RAY_QUERY: &str =
    include_str!(concat!(env!("OUT_DIR"), "/quality_ao_ray_query.wgsl"));

/// [`QUALITY_AO_RAY_QUERY`] with the visual resolver replaced by generated code.
pub const QUALITY_AO_RAY_QUERY_SPECIALIZED: &str = include_str!(concat!(
    env!("OUT_DIR"),
    "/quality_ao_ray_query.specialized.wgsl"
));

/// Gbuffer sphere impostors.
pub const GEOMETRY_SPHERE: &str = include_str!(concat!(env!("OUT_DIR"), "/sphere.wgsl"));

/// [`GEOMETRY_SPHERE`] with the visual resolver replaced by generated code.
pub const GEOMETRY_SPHERE_SPECIALIZED: &str =
    include_str!(concat!(env!("OUT_DIR"), "/sphere.specialized.wgsl"));

/// Gbuffer analytic bond capsules.
pub const GEOMETRY_BOND: &str = include_str!(concat!(env!("OUT_DIR"), "/bond.wgsl"));

/// [`GEOMETRY_BOND`] with the visual resolver replaced by generated code.
pub const GEOMETRY_BOND_SPECIALIZED: &str =
    include_str!(concat!(env!("OUT_DIR"), "/bond.specialized.wgsl"));

/// Transport-frame cartoon geometry.
pub const GEOMETRY_CARTOON: &str = include_str!(concat!(env!("OUT_DIR"), "/cartoon.wgsl"));

/// [`GEOMETRY_CARTOON`] with the visual resolver replaced by generated code.
pub const GEOMETRY_CARTOON_SPECIALIZED: &str =
    include_str!(concat!(env!("OUT_DIR"), "/cartoon.specialized.wgsl"));

/// BVH-bounded implicit molecular surfaces.
pub const GEOMETRY_SURFACE: &str = include_str!(concat!(env!("OUT_DIR"), "/surface.wgsl"));

/// [`GEOMETRY_SURFACE`] with the visual resolver replaced by generated code.
pub const GEOMETRY_SURFACE_SPECIALIZED: &str =
    include_str!(concat!(env!("OUT_DIR"), "/surface.specialized.wgsl"));

/// Analytic caller-authored ellipsoids, carbohydrate symbols and planes.
pub const GEOMETRY_PRIMITIVE: &str = include_str!(concat!(env!("OUT_DIR"), "/primitive.wgsl"));

/// Compact reusable-topology ligand pose impostors.
pub const GEOMETRY_LIGAND_POSE: &str = include_str!(concat!(env!("OUT_DIR"), "/ligand_pose.wgsl"));

/// Scene-fit analytic primitive shadow map.
pub const SHADOW: &str = include_str!(concat!(env!("OUT_DIR"), "/shadow.wgsl"));

/// [`SHADOW`] with the visual resolver replaced by generated code.
pub const SHADOW_SPECIALIZED: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shadow.specialized.wgsl"));

/// Depth-only compact ligand-pose impostors.
pub const LIGAND_POSE_SHADOW: &str =
    include_str!(concat!(env!("OUT_DIR"), "/ligand_pose_shadow.wgsl"));

/// Depth-only cartoon-ribbon shadow caster, pulled from ribbon storage.
pub const SHADOW_RIBBON: &str = include_str!(concat!(env!("OUT_DIR"), "/shadow_ribbon.wgsl"));

/// [`SHADOW_RIBBON`] with the visual resolver replaced by generated code.
pub const SHADOW_RIBBON_SPECIALIZED: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shadow_ribbon.specialized.wgsl"));

/// Pixel-stable circular atom points.
pub const GEOMETRY_POINT: &str = include_str!(concat!(env!("OUT_DIR"), "/point.wgsl"));

/// [`GEOMETRY_POINT`] with the visual resolver replaced by generated code.
pub const GEOMETRY_POINT_SPECIALIZED: &str =
    include_str!(concat!(env!("OUT_DIR"), "/point.specialized.wgsl"));

/// Generic analytic points over tightly packed caller positions.
pub const GENERIC_POINT: &str = include_str!(concat!(env!("OUT_DIR"), "/generic_point.wgsl"));

/// [`GENERIC_POINT`] with the visual resolver replaced by generated code.
pub const GENERIC_POINT_SPECIALIZED: &str =
    include_str!(concat!(env!("OUT_DIR"), "/generic_point.specialized.wgsl"));

/// Generic point culling and indirect argument generation.
pub const GENERIC_POINT_CULL: &str =
    include_str!(concat!(env!("OUT_DIR"), "/generic_point_cull.wgsl"));

/// Shared analytic templates expanded from compact rigid transforms.
pub const GENERIC_INSTANCE: &str = include_str!(concat!(env!("OUT_DIR"), "/generic_instance.wgsl"));

/// [`GENERIC_INSTANCE`] with the visual resolver replaced by generated code.
pub const GENERIC_INSTANCE_SPECIALIZED: &str = include_str!(concat!(
    env!("OUT_DIR"),
    "/generic_instance.specialized.wgsl"
));

/// Rigid-instance culling and indirect analytic draw generation.
pub const GENERIC_INSTANCE_CULL: &str =
    include_str!(concat!(env!("OUT_DIR"), "/generic_instance_cull.wgsl"));

/// Optional shared interpolation for repeatedly consumed rigid instances.
pub const INSTANCE_TIMELINE: &str =
    include_str!(concat!(env!("OUT_DIR"), "/instance_timeline.wgsl"));

/// Optional shared interpolation for deformable point positions.
pub const POINT_TIMELINE: &str = include_str!(concat!(env!("OUT_DIR"), "/point_timeline.wgsl"));

/// Optional shared interpolation for scalar and vector attribute frames.
pub const ATTRIBUTE_TIMELINE: &str =
    include_str!(concat!(env!("OUT_DIR"), "/attribute_timeline.wgsl"));

/// Branch-free dynamic relation endpoint resolution.
pub const RELATION_RESOLVE: &str = include_str!(concat!(env!("OUT_DIR"), "/relation_resolve.wgsl"));

/// Relation visibility compaction and indirect argument generation.
pub const RELATION_CULL: &str = include_str!(concat!(env!("OUT_DIR"), "/relation_cull.wgsl"));

/// Batched provider-backed point chunks.
pub const PAGED_CHUNK: &str = include_str!(concat!(env!("OUT_DIR"), "/paged_chunk.wgsl"));

/// Batched provider-backed analytic bonds over resident atom pages.
pub const PAGED_BOND: &str = include_str!(concat!(env!("OUT_DIR"), "/paged_bond.wgsl"));

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

/// Compact continuous normals generated from the final molecular field.
pub const SURFACE_FIELD_NORMAL: &str =
    include_str!(concat!(env!("OUT_DIR"), "/surface_field_normal.wgsl"));

/// Working-set connected-component labeling and sampled-field filtering.
pub const SURFACE_COMPONENT_FILTER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/surface_component_filter.wgsl"));

/// Visibility compaction and indirect argument generation.
pub const CULL: &str = include_str!(concat!(env!("OUT_DIR"), "/cull.wgsl"));

/// Bounded typed visual-program entity evaluator.
pub const VISUAL_PROGRAM: &str = include_str!(concat!(env!("OUT_DIR"), "/visual_program.wgsl"));

/// Topology-stable two-frame coordinate interpolation.
pub const TRAJECTORY: &str = include_str!(concat!(env!("OUT_DIR"), "/trajectory.wgsl"));

/// GPU-resident temporal occupancy accumulation and volume resolution.
pub const OCCUPANCY: &str = include_str!(concat!(env!("OUT_DIR"), "/occupancy.wgsl"));
/// Occupancy shader specialized for an RGBA32 bounds texture.
pub const OCCUPANCY_RGBA: &str = include_str!(concat!(env!("OUT_DIR"), "/occupancy_rgba.wgsl"));
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

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "sources_gl_tests.rs"]
mod gl_tests;
