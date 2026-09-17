// Analytic generic discs and spheres over tightly packed 12-byte positions.

//!include "include/camera.wgsl"
//!include "include/quad.wgsl"
//!include "include/intersect.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/oit_input.wgsl"
//!include "include/depth_tie.wgsl"
//!include "include/visual/program_types.wgsl"

struct GenericPointConfig {
    source: vec4u,
    picking: vec4u,
    timeline: vec4f,
}

struct GenericPointVertex {
    @builtin(position) position: vec4f,
    @location(0) ray_xy: vec2f,
    @location(1) @interpolate(flat, first) center_radius: vec4f,
    @location(2) @interpolate(flat, first) color: vec4f,
    @location(3) @interpolate(flat, first) identity: vec3u,
}

struct GenericPointOutput {
    @location(0) albedo_material: vec4f,
    @location(1) normal_roughness: vec4f,
    @location(2) entity_id: u32,
    @location(3) resident_page: u32,
    @location(4) motion: vec2f,
    @builtin(frag_depth) depth: f32,
}

struct PointCullOutput {
    args: vec4u,
    visible: array<u32>,
}

@group(2) @binding(0) var<storage, read> point_positions: array<u32>;
@group(2) @binding(1) var<storage, read> point_output: PointCullOutput;
@group(2) @binding(2) var<uniform> point_config: GenericPointConfig;
@group(2) @binding(3) var<storage, read> visual_properties: array<u32>;
@group(2) @binding(4) var<storage, read> visual_results: array<u32>;
@group(2) @binding(5) var<uniform> visual_config: VisualConfig;

struct VisualFragmentProgram {
    instructions: array<VisualInstruction, 64>,
    parameters: array<vec4f, 16>,
}

@group(2) @binding(6) var<uniform> visual_fragment_program: VisualFragmentProgram;
@group(2) @binding(7) var<storage, read> point_positions_end: array<u32>;

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

fn generic_point_position(index: u32) -> vec3f {
    let base = index * 3u;
    let start = vec3f(
        bitcast<f32>(point_positions[base]),
        bitcast<f32>(point_positions[base + 1u]),
        bitcast<f32>(point_positions[base + 2u]),
    );
    let end = vec3f(
        bitcast<f32>(point_positions_end[base]),
        bitcast<f32>(point_positions_end[base + 1u]),
        bitcast<f32>(point_positions_end[base + 2u]),
    );
    return mix(start, end, point_config.timeline.x);
}

fn generic_point_presented_position(index: u32) -> vec3f {
    return generic_point_position(index) + visual_result_offset(index);
}

fn generic_point_geometry(index: u32) -> vec4f {
    return unpack4x8unorm(visual_result_word(
        index,
        VISUAL_RESULT_GEOMETRY,
        pack4x8unorm(visual_config.uniform_geometry),
    ));
}

fn generic_point_color(index: u32) -> vec4f {
    let fallback = visual_uniform_base_color(unpack4x8unorm(point_config.source.z));
    return unpack4x8unorm(visual_result_word(
        index,
        VISUAL_RESULT_COLOR,
        pack4x8unorm(fallback),
    ));
}

fn generic_point_fragment(
    row: u32,
    base_color: vec4f,
    world_position: vec3f,
    world_normal: vec3f,
) -> VisualFragmentResult {
    let response = unpack4x8unorm(visual_result_word(
        row,
        VISUAL_RESULT_RESPONSE,
        pack4x8unorm(vec4f(
            visual_config.material.y,
            visual_config.material.z,
            visual_config.material.w,
            0.0,
        )),
    ));
    let geometry = generic_point_geometry(row);
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
    let view_direction = select(
        camera_delta / camera_distance,
        vec3f(0.0),
        camera_distance <= 1.0e-8 || camera_distance != camera_distance,
    );
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

@vertex
fn vs_generic_point(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> GenericPointVertex {
    let row = point_output.visible[instance];
    let world_center = generic_point_presented_position(row);
    let center = camera_view_position(world_center);
    let radius = bitcast<f32>(point_config.source.y) * generic_point_geometry(row).z * 4.0;
    var half_size = vec2f(radius);
    if point_config.source.w == 1u && frame.projection_kind.x < 0.5 {
        half_size = sphere_quad_half_extent(center, radius);
    }
    let view_position = center + vec3f(quad_corner(vertex) * half_size, 0.0);
    var out: GenericPointVertex;
    out.position = camera_view_clip(view_position);
    out.ray_xy = view_position.xy;
    out.center_radius = vec4f(center, radius);
    out.color = generic_point_color(row);
    out.identity = vec3u(row, point_config.picking.x, point_config.source.w);
    return out;
}

fn generic_point_surface(in: GenericPointVertex) -> vec4f {
    if in.identity.z == 0u {
        let delta = (in.ray_xy - in.center_radius.xy) / in.center_radius.w;
        if dot(delta, delta) > 1.0 {
            return vec4f(0.0);
        }
        return vec4f(in.center_radius.xyz, 1.0);
    }
    var origin = vec3f(0.0);
    var direction = vec3f(in.ray_xy, in.center_radius.z);
    if frame.projection_kind.x > 0.5 {
        origin = vec3f(in.ray_xy, 0.0);
        direction = vec3f(0.0, 0.0, -1.0);
    }
    let center = in.center_radius.xyz - origin;
    let t = ray_sphere(direction, center, in.center_radius.w);
    if t <= 0.0 {
        return vec4f(0.0);
    }
    return vec4f(origin + direction * t, 1.0);
}

@fragment
fn fs_generic_point(in: GenericPointVertex) -> GenericPointOutput {
    let surface = generic_point_surface(in);
    if surface.w == 0.0 {
        discard;
    }
    var normal = vec3f(0.0, 0.0, 1.0);
    if in.identity.z == 1u {
        normal = normalize(surface.xyz - in.center_radius.xyz);
    }
    let world_position = camera_world_position(surface.xyz);
    let visual = generic_point_fragment(
        in.identity.x,
        in.color,
        world_position,
        camera_world_direction(normal),
    );
    if !visual.visible || visual.color.a <= 0.0 {
        discard;
    }
    var out: GenericPointOutput;
    out.albedo_material = vec4f(
        visual.color.rgb + visual.emission,
        material_payload(vec4f(visual.roughness, visual.specular, 0.0, visual.material_strength))
            + select(0.0, 8.0, visual.emission_enabled),
    );
    out.normal_roughness = vec4f(
        encode_shading_frame(normal, canonical_tangent(normal)),
        visual.roughness,
    );
    out.entity_id = in.identity.x;
    out.resident_page = in.identity.y;
    out.motion = vec2f(0.0);
    out.depth = stable_entity_depth(camera_view_depth(surface.xyz), in.identity.x);
    return out;
}

@fragment
fn fs_generic_point_transparent(in: GenericPointVertex) -> OitOutput {
    let surface = generic_point_surface(in);
    if surface.w == 0.0 {
        discard;
    }
    var normal = vec3f(0.0, 0.0, 1.0);
    if in.identity.z == 1u {
        normal = normalize(surface.xyz - in.center_radius.xyz);
    }
    let world_position = camera_world_position(surface.xyz);
    let visual = generic_point_fragment(
        in.identity.x,
        in.color,
        world_position,
        camera_world_direction(normal),
    );
    if !visual.visible || visual.color.a <= 1.0e-4 {
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
        normal,
        visual.roughness,
        material,
        surface.xyz,
        oit_occlusion(in.position),
    ) + visual.emission;
    return weighted_transparency(
        lit,
        visual.color.a,
        camera_view_depth(surface.xyz),
    );
}
