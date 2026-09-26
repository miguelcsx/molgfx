// Batched provider-backed atom chunks with chunk-local u32 addressing.

//!include "include/camera.wgsl"
//!include "include/quad.wgsl"
//!include "include/intersect.wgsl"
//!include "include/surface_frame.wgsl"

struct PlacementRecord {
    model_to_world: mat4x4f,
    coordinate_base: u32,
    radius_base: u32,
    cluster_base: u32,
    cluster_count: u32,
    local_rows: u32,
    pick_page: u32,
    color: u32,
    representation: u32,
    size: f32,
    cluster_padding: f32,
    dynamic_coordinates: u32,
    padding: u32,
}

struct VisibleRecord {
    placement: u32,
    local_row: u32,
}

struct IndirectCommand {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: u32,
}

struct TrajectoryWindow {
    output_base: u32,
    start_base: u32,
    end_base: u32,
    count: u32,
    interpolation: f32,
    padding: vec3u,
}

@group(1) @binding(0) var<storage, read_write> paged_coordinates: array<f32>;
@group(1) @binding(1) var<storage, read> paged_clusters: array<vec4f>;
@group(1) @binding(2) var<storage, read> paged_placements: array<PlacementRecord>;
@group(1) @binding(3) var<storage, read_write> paged_visible_out: array<VisibleRecord>;
@group(1) @binding(4) var<storage, read_write> paged_commands: array<IndirectCommand>;
@group(1) @binding(5) var<storage, read> paged_visible: array<VisibleRecord>;
@group(1) @binding(6) var<storage, read> trajectory_frames: array<f32>;
@group(1) @binding(7) var<storage, read> trajectory_windows: array<TrajectoryWindow>;
@group(1) @binding(8) var<storage, read> paged_render_coordinates: array<f32>;

var<workgroup> cluster_keep: u32;
var<workgroup> cluster_rows: u32;
var<workgroup> cluster_output_base: u32;

fn transform_point(transform: mat4x4f, point: vec3f) -> vec3f {
    return transform[0].xyz * point.x
        + transform[1].xyz * point.y
        + transform[2].xyz * point.z
        + transform[3].xyz;
}

fn max_scale(transform: mat4x4f) -> f32 {
    return max(length(transform[0].xyz), max(length(transform[1].xyz), length(transform[2].xyz)));
}

fn cluster_visible(placement: PlacementRecord, cluster: vec4f) -> bool {
    let world_center = transform_point(placement.model_to_world, cluster.xyz);
    let view_center = camera_view_position(world_center);
    let clip = camera_view_clip(view_center);
    if clip.w <= 0.0 {
        return false;
    }
    let radius = (cluster.w + placement.cluster_padding)
        * max_scale(placement.model_to_world);
    let projected = radius * max(abs(frame.proj[0].x), abs(frame.proj[1].y))
        / max(abs(view_center.z), 1.0e-6);
    let ndc = clip.xy / clip.w;
    return abs(ndc.x) <= 1.0 + projected && abs(ndc.y) <= 1.0 + projected;
}

@compute @workgroup_size(64, 1, 1)
fn interpolate_paged_trajectory(@builtin(global_invocation_id) invocation: vec3u) {
    let window = trajectory_windows[invocation.y];
    let row = invocation.x;
    if row >= window.count {
        return;
    }
    let output = window.output_base + row * 3u;
    let start = window.start_base + row * 3u;
    let end = window.end_base + row * 3u;
    let alpha = window.interpolation;
    paged_coordinates[output] = mix(trajectory_frames[start], trajectory_frames[end], alpha);
    paged_coordinates[output + 1u] = mix(
        trajectory_frames[start + 1u], trajectory_frames[end + 1u], alpha,
    );
    paged_coordinates[output + 2u] = mix(
        trajectory_frames[start + 2u], trajectory_frames[end + 2u], alpha,
    );
}

@compute @workgroup_size(1)
fn reset_paged_chunks(@builtin(global_invocation_id) invocation: vec3u) {
    atomicStore(&paged_commands[invocation.x].instance_count, 0u);
}

@compute @workgroup_size(64, 1, 1)
fn cull_paged_chunks(
    @builtin(workgroup_id) group: vec3u,
    @builtin(local_invocation_index) lane: u32,
) {
    let placement_index = group.y;
    if lane == 0u {
        cluster_keep = 0u;
        cluster_rows = 0u;
        cluster_output_base = 0u;
        let placement = paged_placements[placement_index];
        if group.x < placement.cluster_count {
            let cluster = paged_clusters[placement.cluster_base + group.x];
            let first_row = group.x * 64u;
            if first_row < placement.local_rows
                && (placement.dynamic_coordinates != 0u || cluster_visible(placement, cluster)) {
                let rows = min(64u, placement.local_rows - first_row);
                let command = placement.representation;
                cluster_rows = rows;
                cluster_output_base = paged_commands[command].first_instance
                    + atomicAdd(&paged_commands[command].instance_count, rows);
                cluster_keep = 1u;
            }
        }
    }
    workgroupBarrier();
    if cluster_keep == 0u || lane >= cluster_rows {
        return;
    }
    let local_row = group.x * 64u + lane;
    let output = cluster_output_base + lane;
    if output < arrayLength(&paged_visible_out) {
        paged_visible_out[output] = VisibleRecord(placement_index, local_row);
    }
}

struct PointVertex {
    @builtin(position) position: vec4f,
    @location(0) corner: vec2f,
    @location(1) @interpolate(flat, either) color: u32,
    @location(2) @interpolate(flat, either) local_row: u32,
    @location(3) @interpolate(flat, either) pick_page: u32,
}

@vertex
fn paged_point_vertex(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> PointVertex {
    let visible = paged_visible[instance];
    let placement = paged_placements[visible.placement];
    let base = placement.coordinate_base + visible.local_row * 3u;
    let local = vec3f(
        paged_render_coordinates[base],
        paged_render_coordinates[base + 1u],
        paged_render_coordinates[base + 2u],
    );
    let clip = camera_world_clip(transform_point(placement.model_to_world, local));
    let corner = quad_corner(vertex);
    var out: PointVertex;
    out.position = vec4f(
        clip.xy + corner * placement.size * frame.viewport.zw * clip.w,
        clip.zw,
    );
    out.corner = corner;
    out.color = placement.color;
    out.local_row = visible.local_row;
    out.pick_page = placement.pick_page;
    return out;
}

struct PointFragment {
    @location(0) albedo_material: vec4f,
    @location(1) normal_roughness: vec4f,
    @location(2) local_row: u32,
    @location(3) pick_page: u32,
    @location(4) motion: vec2f,
}

@fragment
fn paged_point_fragment(in: PointVertex) -> PointFragment {
    if dot(in.corner, in.corner) > 1.0 {
        discard;
    }
    var out: PointFragment;
    out.albedo_material = vec4f(unpack4x8unorm(in.color).rgb, 0.0);
    out.normal_roughness = vec4f(0.5, 0.5, 1.0, 0.7);
    out.local_row = in.local_row;
    out.pick_page = in.pick_page;
    out.motion = vec2f(0.0);
    return out;
}

struct PagedSphereVertex {
    @builtin(position) position: vec4f,
    @location(0) ray_xy: vec2f,
    @location(1) @interpolate(flat, either) center_radius: vec4f,
    @location(2) @interpolate(flat, either) color: u32,
    @location(3) @interpolate(flat, either) local_row: u32,
    @location(4) @interpolate(flat, either) pick_page: u32,
}

@vertex
fn paged_spacefill_vertex(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> PagedSphereVertex {
    let visible = paged_visible[instance];
    let placement = paged_placements[visible.placement];
    let coordinate = placement.coordinate_base + visible.local_row * 3u;
    let local_center = vec3f(
        paged_render_coordinates[coordinate],
        paged_render_coordinates[coordinate + 1u],
        paged_render_coordinates[coordinate + 2u],
    );
    let world_center = transform_point(placement.model_to_world, local_center);
    let view_center = camera_view_position(world_center);
    let radius = paged_render_coordinates[placement.radius_base + visible.local_row]
        * placement.size * max_scale(placement.model_to_world);
    var half_extent = vec2f(radius);
    if frame.projection_kind.x < 0.5 {
        half_extent = sphere_quad_half_extent(view_center, radius);
    }
    let view_position = view_center + vec3f(quad_corner(vertex) * half_extent, 0.0);
    var out: PagedSphereVertex;
    out.position = camera_view_clip(view_position);
    out.ray_xy = view_position.xy;
    out.center_radius = vec4f(view_center, radius);
    out.color = placement.color;
    out.local_row = visible.local_row;
    out.pick_page = placement.pick_page;
    return out;
}

struct PagedSphereFragment {
    @location(0) albedo_material: vec4f,
    @location(1) normal_roughness: vec4f,
    @location(2) local_row: u32,
    @location(3) pick_page: u32,
    @location(4) motion: vec2f,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn paged_spacefill_fragment(in: PagedSphereVertex) -> PagedSphereFragment {
    var origin = vec3f(0.0);
    var direction = vec3f(in.ray_xy, in.center_radius.z);
    if frame.projection_kind.x > 0.5 {
        origin = vec3f(in.ray_xy, 0.0);
        direction = vec3f(0.0, 0.0, -1.0);
    }
    let hit_distance = ray_sphere(
        direction,
        in.center_radius.xyz - origin,
        in.center_radius.w,
    );
    if hit_distance <= 0.0 {
        discard;
    }
    let hit = origin + direction * hit_distance;
    let normal = normalize(hit - in.center_radius.xyz);
    var out: PagedSphereFragment;
    out.albedo_material = vec4f(unpack4x8unorm(in.color).rgb, 0.0);
    out.normal_roughness = vec4f(
        encode_shading_frame(normal, canonical_tangent(normal)),
        0.45,
    );
    out.local_row = in.local_row;
    out.pick_page = in.pick_page;
    out.motion = vec2f(0.0);
    out.depth = camera_view_depth(hit);
    return out;
}
