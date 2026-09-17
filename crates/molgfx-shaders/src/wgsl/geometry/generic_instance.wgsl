// Shared analytic templates expanded only for GPU-compacted rigid instances.

//!include "include/camera.wgsl"
//!include "include/quad.wgsl"
//!include "include/intersect.wgsl"
//!include "include/quaternion.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/oit_input.wgsl"
//!include "include/motion.wgsl"
//!include "include/primitive/types.wgsl"
//!include "include/primitive/box.wgsl"
//!include "include/particle.wgsl"
//!include "include/primitive/output.wgsl"
//!include "include/visual/program_types.wgsl"

const GENERIC_INSTANCE_SPHERE: u32 = 0u;
const GENERIC_INSTANCE_CAPSULE: u32 = 3u;
override GENERIC_INSTANCE_SHAPE: u32 = GENERIC_INSTANCE_SPHERE;

struct RigidInstanceGpu {
    translation_scale: vec4f,
    orientation: vec4f,
}

struct GenericCapsuleGpu {
    center_length: vec4f,
    orientation: vec4f,
    radius_reserved: vec4f,
}

struct GenericInstanceConfig {
    counts: vec4u,
    picking_style: vec4u,
    bounds: vec4f,
}

struct InstanceCullOutput {
    args: array<vec4u, 2>,
    visible: array<u32>,
}

@group(2) @binding(5) var<storage, read> template_spheres: array<vec4f>;
@group(2) @binding(6) var<storage, read> template_capsules: array<GenericCapsuleGpu>;
@group(2) @binding(7) var<storage, read> instance_transforms: array<RigidInstanceGpu>;
@group(2) @binding(8) var<storage, read> instance_output: InstanceCullOutput;
@group(2) @binding(9) var<uniform> instance_config: GenericInstanceConfig;
@group(2) @binding(10) var<storage, read> visual_properties: array<u32>;
@group(2) @binding(11) var<storage, read> visual_results: array<u32>;
@group(2) @binding(12) var<uniform> visual_config: VisualConfig;

struct VisualFragmentProgram {
    instructions: array<VisualInstruction, 64>,
    parameters: array<vec4f, 16>,
}

@group(2) @binding(13) var<uniform> visual_fragment_program: VisualFragmentProgram;
@group(2) @binding(14) var<storage, read> instance_frame_start: array<RigidInstanceGpu>;
@group(2) @binding(15) var<storage, read> instance_frame_end: array<RigidInstanceGpu>;

fn sampled_instance(index: u32) -> RigidInstanceGpu {
    if instance_config.bounds.z < 0.5 {
        return instance_transforms[instance_config.picking_style.z + index];
    }
    let start = instance_frame_start[instance_config.picking_style.w + index];
    let end = instance_frame_end[bitcast<u32>(instance_config.bounds.w) + index];
    var end_orientation = end.orientation;
    if dot(start.orientation, end_orientation) < 0.0 {
        end_orientation = -end_orientation;
    }
    return RigidInstanceGpu(
        mix(start.translation_scale, end.translation_scale, instance_config.bounds.y),
        normalize(mix(start.orientation, end_orientation, instance_config.bounds.y)),
    );
}

override VISUAL_PROGRAM_ENABLED: bool = false;
override VISUAL_FRAGMENT_ENABLED: bool = false;

fn visual_instruction(index: u32) -> VisualInstruction {
    return visual_fragment_program.instructions[index];
}

fn visual_parameter(index: u32) -> vec4f {
    return visual_fragment_program.parameters[index];
}

//!include "include/visual/evaluator.wgsl"
//!include "include/visual/resolve.wgsl"

fn generic_instance_geometry(index: u32) -> vec4f {
    return unpack4x8unorm(visual_result_word(
        index,
        VISUAL_RESULT_GEOMETRY,
        pack4x8unorm(visual_config.uniform_geometry),
    ));
}

fn generic_instance_color(index: u32) -> vec4f {
    let fallback = visual_uniform_base_color(unpack4x8unorm(instance_config.picking_style.y));
    return unpack4x8unorm(visual_result_word(
        index,
        VISUAL_RESULT_COLOR,
        pack4x8unorm(fallback),
    ));
}

fn generic_instance_fragment(
    row: u32,
    base_color: vec4f,
    world_position: vec3f,
    world_normal: vec3f,
) -> VisualFragmentResult {
    let response = unpack4x8unorm(visual_result_word(
        row,
        VISUAL_RESULT_RESPONSE,
        pack4x8unorm(vec4f(visual_config.material.y, visual_config.material.z, visual_config.material.w, 0.0)),
    ));
    let geometry = generic_instance_geometry(row);
    let fallback = VisualFragmentResult(
        base_color,
        unpack4x8unorm(visual_result_word(
            row,
            VISUAL_RESULT_EMISSION,
            pack4x8unorm(visual_config.uniform_emission / 64.0),
        )).rgb * 64.0,
        visual_config.outputs0.z != VISUAL_MISSING,
        response.x,
        response.y,
        response.z,
        geometry.x > 0.5,
        geometry.y * 8.0,
    );
    if !VISUAL_FRAGMENT_ENABLED || visual_config.counts.z == 0u {
        return fallback;
    }
    let camera_position = frame.inv_view[3].xyz;
    let camera_delta = camera_position - world_position;
    let camera_distance = length(camera_delta);
    let view_direction = select(camera_delta / camera_distance, vec3f(0.0), camera_distance <= 1.0e-8 || camera_distance != camera_distance);
    return visual_resolve(
        VisualEvaluationInputs(
            base_color,
            vec4f(world_position, 0.0),
            vec4f(world_position, 0.0),
            vec4f(world_normal, 0.0),
            vec4f(view_direction, 0.0),
            camera_distance,
            row,
        ),
        fallback,
    );
}

fn quaternion_mul(left: vec4f, right: vec4f) -> vec4f {
    return vec4f(
        left.w * right.xyz
            + right.w * left.xyz
            + cross(left.xyz, right.xyz),
        left.w * right.w - dot(left.xyz, right.xyz),
    );
}

@vertex
fn vs_generic_instance(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) expanded: u32,
) -> PrimitiveVsOut {
    let sphere = GENERIC_INSTANCE_SHAPE == GENERIC_INSTANCE_SPHERE;
    let topology_count = select(instance_config.counts.z, instance_config.counts.y, sphere);
    let visible_slot = expanded / topology_count;
    let part = expanded % topology_count;
    let instance_row = instance_output.visible[visible_slot];
    let transform = sampled_instance(instance_row);
    let geometry = generic_instance_geometry(instance_row);
    let scale = transform.translation_scale.w * geometry.z * 4.0;

    var local_center = vec3f(0.0);
    var local_orientation = vec4f(0.0, 0.0, 0.0, 1.0);
    var local_size = vec3f(0.0);
    var bound_radius = 0.0;
    var flat_part = part;
    if sphere {
        let value = template_spheres[part];
        local_center = value.xyz;
        bound_radius = value.w * scale;
        local_size = vec3f(bound_radius * 2.0);
    } else {
        let value = template_capsules[part];
        local_center = value.center_length.xyz;
        local_orientation = value.orientation;
        let radius = value.radius_reserved.x * scale;
        let length = value.center_length.w * scale;
        bound_radius = length * 0.5 + radius;
        local_size = vec3f(radius * 2.0, radius * 2.0, length + radius * 2.0);
        flat_part += instance_config.counts.y;
    }

    let world_center = transform.translation_scale.xyz
        + visual_result_offset(instance_row)
        + rotate_vector(transform.orientation, local_center * scale);
    let orientation = quaternion_mul(transform.orientation, local_orientation);
    let center = transform_point(frame.view, world_center);
    var half_size = vec2f(bound_radius);
    if frame.projection_kind.x < 0.5 {
        half_size = sphere_quad_half_extent(center, bound_radius);
    }
    let local_vertex = vertex % 6u;
    let view_position = center + vec3f(primitive_corner(local_vertex) * half_size, 0.0);
    let flattened = instance_row * instance_config.counts.w + flat_part;

    var out: PrimitiveVsOut;
    out.position = frame.proj * vec4f(view_position, 1.0);
    out.view_position = view_position;
    out.world_center = vec3f(0.0);
    out.radius = 0.0;
    out.orientation = vec4f(0.0);
    out.size = vec3f(0.0);
    out.inverse_primary = vec4f(0.0);
    out.inverse_cross = vec4f(0.0);
    out.color = vec4f(0.0);
    out.metadata = vec4u(0u);
    out.previous_world_center = world_center;
    if primitive_flat_source(local_vertex) {
        out.world_center = world_center;
        out.radius = bound_radius;
        out.orientation = orientation;
        out.size = local_size;
        out.color = generic_instance_color(instance_row);
        out.metadata = vec4u(flattened, instance_config.picking_style.x, instance_row, flat_part);
    }
    return out;
}

@fragment
fn fs_generic_instance(in: PrimitiveVsOut) -> PrimitiveFsOut {
    let ray = primitive_ray(in.view_position);
    let hit = particle_hit(in, ray.origin, ray.direction);
    if !hit.valid {
        discard;
    }
    let world_position = fma(ray.direction, vec3f(hit.t), ray.origin);
    let view_position = transform_point(frame.view, world_position);
    let view_normal = normalize(transform_direction(frame.view, hit.normal_world));
    let visual = generic_instance_fragment(in.metadata.z, in.color, world_position, hit.normal_world);
    if !visual.visible || visual.color.a <= 0.0 {
        discard;
    }
    var out: PrimitiveFsOut;
    out.albedo_material = vec4f(
        visual.color.rgb + visual.emission,
        material_payload(vec4f(visual.roughness, visual.specular, 0.0, visual.material_strength))
            + select(0.0, 8.0, visual.emission_enabled),
    );
    out.normal_roughness = vec4f(
        encode_shading_frame(view_normal, canonical_tangent(view_normal)),
        visual.roughness,
    );
    out.entity_id = in.metadata.x & 0x0fffffffu;
    out.resident_page = in.metadata.y;
    out.motion = vec2f(0.0);
    out.depth = primitive_view_depth(view_position);
    return out;
}

@fragment
fn fs_generic_instance_transparent(in: PrimitiveVsOut) -> OitOutput {
    let ray = primitive_ray(in.view_position);
    let hit = particle_hit(in, ray.origin, ray.direction);
    if !hit.valid {
        discard;
    }
    let world_position = fma(ray.direction, vec3f(hit.t), ray.origin);
    let view_position = transform_point(frame.view, world_position);
    let view_normal = normalize(transform_direction(frame.view, hit.normal_world));
    let visual = generic_instance_fragment(
        in.metadata.z,
        in.color,
        world_position,
        hit.normal_world,
    );
    let opacity = visual.color.a * hit.weight;
    if !visual.visible || opacity <= 1.0e-4 {
        discard;
    }
    let material = material_payload(vec4f(
        visual.roughness,
        visual.specular,
        0.0,
        visual.material_strength,
    ));
    let lit = shade_molecule(
        visual.color.rgb,
        view_normal,
        visual.roughness,
        material,
        view_position,
        oit_occlusion(in.position),
    ) + visual.emission;
    return weighted_transparency(lit, opacity, primitive_view_depth(view_position));
}
