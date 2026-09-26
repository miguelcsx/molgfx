// One bounded indirect batch of provider bonds referencing resident atom pages.

//!include "include/camera.wgsl"
//!include "include/intersect.wgsl"
//!include "include/surface_frame.wgsl"

const PAGED_BOND_AXIS_EPSILON_SQ: f32 = 1.0e-12;
const PAGED_BOND_MIN_VIEW_DEPTH: f32 = 1.0e-4;

struct PagedBondRecord {
    coordinate_a: u32,
    coordinate_b: u32,
    padding: vec2u,
}

struct PagedBondPlacement {
    model_to_world: mat4x4f,
    bond_base: u32,
    bond_count: u32,
    pick_page: u32,
    color: u32,
    radius: f32,
    padding: vec3u,
}

struct PagedBondCommand {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: u32,
}

struct PagedVisibleBond {
    placement: u32,
    local_row: u32,
}

var<workgroup> bond_keep: array<u32, 64>;
var<workgroup> bond_prefix: array<u32, 64>;
var<workgroup> bond_output_base: u32;

@group(1) @binding(0) var<storage, read> bond_coordinates: array<f32>;
@group(1) @binding(1) var<storage, read> paged_bonds: array<PagedBondRecord>;
@group(1) @binding(2) var<storage, read> bond_placements: array<PagedBondPlacement>;
@group(1) @binding(3) var<storage, read_write> visible_bond_out: array<PagedVisibleBond>;
@group(1) @binding(4) var<storage, read_write> paged_bond_command: PagedBondCommand;
@group(1) @binding(5) var<storage, read> visible_bonds: array<PagedVisibleBond>;

fn paged_coordinate(base: u32) -> vec3f {
    return vec3f(
        bond_coordinates[base],
        bond_coordinates[base + 1u],
        bond_coordinates[base + 2u],
    );
}

fn paged_transform(transform: mat4x4f, point: vec3f) -> vec3f {
    return transform[0].xyz * point.x
        + transform[1].xyz * point.y
        + transform[2].xyz * point.z
        + transform[3].xyz;
}

fn paged_scale(transform: mat4x4f) -> f32 {
    return max(length(transform[0].xyz), max(length(transform[1].xyz), length(transform[2].xyz)));
}

fn paged_bond_view_position(world: vec3f) -> vec3f {
    return frame.view[0].xyz * world.x
        + frame.view[1].xyz * world.y
        + frame.view[2].xyz * world.z
        + frame.view[3].xyz;
}

fn paged_bond_project_ndc(view: vec3f) -> vec2f {
    let xyw = frame.proj[0].xyw * view.x
        + frame.proj[1].xyw * view.y
        + frame.proj[2].xyw * view.z
        + frame.proj[3].xyw;
    return xyw.xy / xyw.z;
}

fn paged_bond_ray_xy(ndc: vec2f) -> vec2f {
    if frame.projection_kind.x > 0.5 {
        return (ndc - frame.proj[3].xy) / vec2f(frame.proj[0][0], frame.proj[1][1]);
    }
    return (ndc + frame.proj[2].xy) / vec2f(frame.proj[0][0], frame.proj[1][1]);
}

fn paged_bond_view_depth(view: vec3f) -> f32 {
    let zw = frame.proj[0].zw * view.x
        + frame.proj[1].zw * view.y
        + frame.proj[2].zw * view.z
        + frame.proj[3].zw;
    return zw.x / zw.y;
}

fn paged_bond_quad_uv(vertex: u32) -> vec2f {
    let corners = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
        vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
    );
    return corners[min(vertex, 5u)];
}

fn paged_bond_quad_ndc(a: vec2f, b: vec2f, extent: vec2f, vertex: u32) -> vec2f {
    return mix(min(a, b) - extent, max(a, b) + extent, paged_bond_quad_uv(vertex));
}

fn paged_bond_extent(a: vec3f, b: vec3f, radius: f32) -> vec2f {
    var scale = radius;
    if frame.projection_kind.x < 0.5 {
        scale = radius / max(min(-a.z, -b.z) - radius, PAGED_BOND_MIN_VIEW_DEPTH);
    }
    return abs(vec2f(frame.proj[0][0], frame.proj[1][1])) * scale;
}

@compute @workgroup_size(1)
fn reset_paged_bonds() {
    atomicStore(&paged_bond_command.instance_count, 0u);
}

@compute @workgroup_size(64, 1, 1)
fn cull_paged_bonds(
    @builtin(workgroup_id) group: vec3u,
    @builtin(local_invocation_index) lane: u32,
) {
    let placement_index = group.y;
    let placement = bond_placements[placement_index];
    let local_row = group.x * 64u + lane;
    var keep = 0u;
    if local_row < placement.bond_count {
        let bond = paged_bonds[placement.bond_base + local_row];
        let a = paged_transform(placement.model_to_world, paged_coordinate(bond.coordinate_a));
        let b = paged_transform(placement.model_to_world, paged_coordinate(bond.coordinate_b));
        let clip_a = camera_world_clip(a);
        let clip_b = camera_world_clip(b);
        keep = select(0u, 1u, clip_a.w > 0.0 || clip_b.w > 0.0);
    }
    bond_keep[lane] = keep;
    workgroupBarrier();
    if lane == 0u {
        var total = 0u;
        for (var index = 0u; index < 64u; index += 1u) {
            bond_prefix[index] = total;
            total += bond_keep[index];
        }
        bond_output_base = atomicAdd(&paged_bond_command.instance_count, total);
    }
    workgroupBarrier();
    if keep == 0u {
        return;
    }
    let output = bond_output_base + bond_prefix[lane];
    if output < arrayLength(&visible_bond_out) {
        visible_bond_out[output] = PagedVisibleBond(placement_index, local_row);
    }
}

struct PagedBondVertex {
    @builtin(position) position: vec4f,
    @location(0) ray_xy: vec2f,
    @location(1) @interpolate(flat, either) endpoint_a_radius: vec4f,
    @location(2) @interpolate(flat, either) endpoint_b_inv_axis_sq: vec4f,
    @location(3) @interpolate(flat, either) color: u32,
    @location(4) @interpolate(flat, either) local_row: u32,
    @location(5) @interpolate(flat, either) pick_page: u32,
}

@vertex
fn paged_bond_vertex(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> PagedBondVertex {
    let visible = visible_bonds[instance];
    let placement = bond_placements[visible.placement];
    let bond = paged_bonds[placement.bond_base + visible.local_row];
    let world_a = paged_transform(placement.model_to_world, paged_coordinate(bond.coordinate_a));
    let world_b = paged_transform(placement.model_to_world, paged_coordinate(bond.coordinate_b));
    let endpoint_a = paged_bond_view_position(world_a);
    let endpoint_b = paged_bond_view_position(world_b);
    let radius = placement.radius * paged_scale(placement.model_to_world);
    let ndc = paged_bond_quad_ndc(
        paged_bond_project_ndc(endpoint_a),
        paged_bond_project_ndc(endpoint_b),
        paged_bond_extent(endpoint_a, endpoint_b, radius),
        vertex,
    );
    let axis = endpoint_b - endpoint_a;
    var out: PagedBondVertex;
    out.position = vec4f(ndc, 0.0, 1.0);
    out.ray_xy = paged_bond_ray_xy(ndc);
    out.endpoint_a_radius = vec4f(endpoint_a, radius);
    out.endpoint_b_inv_axis_sq = vec4f(
        endpoint_b,
        1.0 / max(dot(axis, axis), PAGED_BOND_AXIS_EPSILON_SQ),
    );
    out.color = placement.color;
    out.local_row = visible.local_row;
    out.pick_page = placement.pick_page;
    return out;
}

struct PagedBondFragment {
    @location(0) albedo_material: vec4f,
    @location(1) normal_roughness: vec4f,
    @location(2) local_row: u32,
    @location(3) pick_page: u32,
    @location(4) motion: vec2f,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn paged_bond_fragment(in: PagedBondVertex) -> PagedBondFragment {
    var origin = vec3f(0.0);
    var direction = vec3f(in.ray_xy, -1.0);
    if frame.projection_kind.x > 0.5 {
        origin = vec3f(in.ray_xy, 0.0);
        direction = vec3f(0.0, 0.0, -1.0);
    }
    let interval = ray_capsule_interval(
        direction,
        in.endpoint_a_radius.xyz - origin,
        in.endpoint_b_inv_axis_sq.xyz - origin,
        in.endpoint_a_radius.w,
    );
    let distance = nearest_positive_interval(interval);
    if distance <= 0.0 {
        discard;
    }
    let hit = origin + direction * distance;
    let axis = in.endpoint_b_inv_axis_sq.xyz - in.endpoint_a_radius.xyz;
    let along = clamp(
        dot(hit - in.endpoint_a_radius.xyz, axis) * in.endpoint_b_inv_axis_sq.w,
        0.0,
        1.0,
    );
    let nearest = in.endpoint_a_radius.xyz + axis * along;
    let normal = normalize(hit - nearest);
    var out: PagedBondFragment;
    out.albedo_material = vec4f(unpack4x8unorm(in.color).rgb, 0.0);
    out.normal_roughness = vec4f(encode_shading_frame(normal, canonical_tangent(normal)), 0.45);
    out.local_row = in.local_row;
    out.pick_page = in.pick_page;
    out.motion = vec2f(0.0);
    out.depth = paged_bond_view_depth(hit);
    return out;
}
