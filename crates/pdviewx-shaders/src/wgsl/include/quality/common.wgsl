// Progressive molecular cavity occlusion and area-light shadows.
//
// One deterministic low-discrepancy sample is traced per covered pixel and
// frame against the same compact atom BVH used by culling and surfaces. The
// HDR temporal pass accumulates the result while the camera is still.

//!include "include/camera.wgsl"
//!include "include/fullscreen.wgsl"
//!include "include/surface_frame.wgsl"
//!include "include/atom.wgsl"
//!include "include/surface_field.wgsl"
//!include "include/intersect.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/visual/fragment.wgsl"
//!include "include/quality/analytic.wgsl"

@group(1) @binding(0) var depth_texture: texture_depth_2d;
@group(1) @binding(1) var normal_texture: texture_2d<f32>;
@group(2) @binding(21) var<storage, read> quality_bond_data: array<u32>;

fn quality_bond_node(index: u32) -> BvhNode {
    let base = index * 8u;
    return BvhNode(
        bitcast<vec4f>(vec4u(
            quality_bond_data[base],
            quality_bond_data[base + 1u],
            quality_bond_data[base + 2u],
            quality_bond_data[base + 3u],
        )),
        bitcast<vec4f>(vec4u(
            quality_bond_data[base + 4u],
            quality_bond_data[base + 5u],
            quality_bond_data[base + 6u],
            quality_bond_data[base + 7u],
        )),
    );
}

fn quality_bond_index(index: u32) -> u32 {
    return quality_bond_data[visual_counts.bond_bvh_nodes * 8u + index];
}

fn quality_bond(index: u32) -> BondRecord {
    let base = visual_counts.bond_bvh_nodes * 8u
        + visual_counts.bond_bvh_indices
        + index * 4u;
    return BondRecord(
        quality_bond_data[base],
        quality_bond_data[base + 1u],
        bitcast<f32>(quality_bond_data[base + 2u]),
        quality_bond_data[base + 3u],
    );
}

const INTERACTIVE_RAYS: u32 = 2u;
const PUBLICATION_RAYS: u32 = 8u;
const AO_DISTANCE: f32 = 12.0;
const SHADOW_DISTANCE: f32 = 40.0;
const AO_STRENGTH: f32 = 0.72;
/// Below this the ray is treated as fully blocked and traversal stops.
const MINIMUM_TRANSMITTANCE: f32 = 0.004;

fn quality_hit_transparency(
    entity_id: u32,
    base_color: vec4f,
    local_position: vec3f,
    local_normal: vec3f,
) -> f32 {
    if !VISUAL_PROGRAM_ENABLED {
        return 1.0 - base_color.a;
    }
    let world_position = atom_transform_point(model.model_to_world, local_position);
    let world_normal = normalize(
        (transpose(model.world_to_model) * vec4f(local_normal, 0.0)).xyz
    );
    let visual = visual_fragment(
        entity_id,
        base_color,
        local_position,
        world_position,
        world_normal,
    );
    return select(1.0, 1.0 - visual.color.a, visual.visible);
}

fn view_position(pixel: vec2i, depth: f32, dimensions: vec2i) -> vec3f {
    let uv = (vec2f(pixel) + 0.5) / vec2f(dimensions);
    let clip = vec4f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let view = frame.inv_proj * clip;
    return view.xyz / max(abs(view.w), 1e-7) * sign(view.w);
}

fn hash_u32(value: u32) -> u32 {
    var state = value;
    state = (state ^ (state >> 16u)) * 0x7feb352du;
    state = (state ^ (state >> 15u)) * 0x846ca68bu;
    return state ^ (state >> 16u);
}

fn random_pair(pixel: vec2i, stream: u32) -> vec2f {
    let sample_index = u32(frame.temporal.w);
    let seed = (u32(pixel.x) * 0x9e3779b9u) ^ (u32(pixel.y) * 0x85ebca6bu)
        ^ (sample_index * 0xc2b2ae35u) ^ stream;
    let first = hash_u32(seed);
    let second = hash_u32(first ^ 0x68bc21ebu);
    return vec2f(f32(first & 0x00ffffffu), f32(second & 0x00ffffffu)) / 16777216.0;
}

fn tangent_frame(normal: vec3f) -> mat2x3f {
    let axis = select(vec3f(0.0, 0.0, 1.0), vec3f(0.0, 1.0, 0.0), abs(normal.z) > 0.9);
    let tangent = normalize(cross(axis, normal));
    return mat2x3f(tangent, cross(normal, tangent));
}

fn cosine_direction(normal: vec3f, random: vec2f) -> vec3f {
    let radius = sqrt(random.x);
    let angle = 6.28318530718 * random.y;
    let frame_axes = tangent_frame(normal);
    return normalize(frame_axes[0] * (radius * cos(angle))
        + frame_axes[1] * (radius * sin(angle))
        + normal * sqrt(max(0.0, 1.0 - random.x)));
}

fn box_interval(origin: vec3f, inverse_direction: vec3f, lower: vec3f, upper: vec3f) -> vec2f {
    let lower_t = (lower - origin) * inverse_direction;
    let upper_t = (upper - origin) * inverse_direction;
    return vec2f(max(max(min(lower_t, upper_t).x, min(lower_t, upper_t).y),
        min(lower_t, upper_t).z), min(min(max(lower_t, upper_t).x,
        max(lower_t, upper_t).y), max(lower_t, upper_t).z));
}

//!include "include/quality/traversal.wgsl"

@fragment
fn fs_quality_ao(in: FullscreenOut) -> @location(0) vec4f {
    let dimensions = vec2i(textureDimensions(depth_texture));
    let pixel = clamp(vec2i(in.position.xy), vec2i(0), dimensions - 1);
    let depth = textureLoad(depth_texture, pixel, 0);
    if depth <= 0.0 {
        return vec4f(0.0);
    }
    let view_point = view_position(pixel, depth, dimensions);
    let view_normal = decode_shading_frame(textureLoad(normal_texture, pixel, 0).xyz).normal;
    let world_point = (frame.inv_view * vec4f(view_point, 1.0)).xyz;
    let world_normal = normalize((frame.inv_view * vec4f(view_normal, 0.0)).xyz);
    let origin = world_point + world_normal * 0.08;
    let light_base = normalize((frame.inv_view * vec4f(frame.lighting[4].xyz, 0.0)).xyz);
    let light_frame = tangent_frame(light_base);
    var ao_hits = 0.0;
    var shadow_hits = 0.0;
    let ray_count = select(INTERACTIVE_RAYS, PUBLICATION_RAYS, frame.temporal.z > 0.5);
    for (var ray_index = 0u; ray_index < ray_count; ray_index += 1u) {
        let stream = ray_index * 0x9e3779b9u;
        let ao_direction = cosine_direction(world_normal,
            random_pair(pixel, 0x51633e2du ^ stream));
        ao_hits += 1.0 - trace_transmittance(origin, ao_direction, AO_DISTANCE);
        let light_random = random_pair(pixel, 0x94d049bbu ^ stream) * 2.0 - 1.0;
        let light_direction = normalize(light_base
            + light_frame[0] * (light_random.x * frame.lighting[5].w)
            + light_frame[1] * (light_random.y * frame.lighting[5].w));
        shadow_hits += 1.0
            - trace_transmittance(origin, light_direction, SHADOW_DISTANCE);
    }
    let inverse_rays = 1.0 / f32(ray_count);
    return vec4f(ao_hits * inverse_rays * AO_STRENGTH,
        shadow_hits * inverse_rays * frame.lighting[3].w, 0.0, 0.0);
}
