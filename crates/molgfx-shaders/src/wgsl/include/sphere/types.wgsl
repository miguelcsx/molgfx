// Sphere payloads, projection and per-vertex impostor geometry.
//
// The quad is sized in the vertex stage to the sphere's projected silhouette,
// because overdraw is the impostor path's only real cost and a loose quad
// pays it on every covered pixel. Projection drops the unused homogeneous
// rows: an atom centre needs affine XYZ and clip ZW, never the full product.

const SPHERE_CAP_TINT: vec3f = vec3f(0.68, 0.76, 0.82);
const SPHERE_CAP_ROUGHNESS: f32 = 0.82;
const SPHERE_CAP_MATERIAL: f32 = 0.05;

const SPHERE_RAY_EPSILON_SQ: f32 = 1.0e-12;
const SPHERE_SOFTNESS_EPSILON: f32 = 1.0e-6;

struct SphereVsOut {
    @builtin(position) position: vec4f,

    // Quad view-space XY. Z is constant for the impostor and reconstructed
    // from center_radius.z in the fragment stage.
    @location(0) ray_xy: vec2f,

    // xyz = view-space center
    // w   = radius
    @location(1) @interpolate(flat, first) center_radius: vec4f,

    @location(2) @interpolate(flat, first) color: vec4f,

    // xyz = previous world-center offset
    // w   = transparent edge softness in pixels
    @location(3) @interpolate(flat, first) previous_softness: vec4f,

    // x = regular roughness
    // y = clipping-cap roughness
    // z = regular material payload
    // w = inverse radius
    @location(4) @interpolate(flat, first) material: vec4f,

    @location(5) @interpolate(flat, first) entity_id: u32,
}

struct SphereGeometry {
    position: vec4f,
    ray_xy: vec2f,
    center_radius: vec4f,
    world_center: vec3f,
}

struct SphereIntersection {
    interval: vec2f,
    perpendicular_sq: f32,
    valid: bool,
}

struct SphereSurface {
    hit: vec3f,
    normal: vec3f,
    perpendicular_sq: f32,
    cap: bool,
    valid: bool,
}

struct SphereMaterial {
    albedo_material: vec4f,
    roughness: f32,
}

struct SphereFsOut {
    @location(0) albedo_material: vec4f,
    @location(1) normal_roughness: vec4f,
    @location(2) entity_id: u32,
    @location(3) resident_page: u32,
    @location(4) motion: vec2f,
    @builtin(frag_depth) depth: f32,
}

/// Returns triangle-list corners in [-1, 1].
fn sphere_corner(vertex: u32) -> vec2f {
    let corners = array<vec2f, 6>(
        vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0),
        vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0),
    );
    return corners[min(vertex, 5u)];
}

/// Returns the first vertex of each independent triangle.
fn sphere_flat_source(vertex: u32) -> bool {
    return vertex == 0u || vertex == 3u;
}

/// Transforms a world-space point to view space without computing W.
fn sphere_view_position(world: vec3f) -> vec3f {
    return frame.view[0].xyz * world.x
        + frame.view[1].xyz * world.y
        + frame.view[2].xyz * world.z
        + frame.view[3].xyz;
}

/// Transforms a view-space point to world space without computing W.
fn sphere_world_position(view: vec3f) -> vec3f {
    return frame.inv_view[0].xyz * view.x
        + frame.inv_view[1].xyz * view.y
        + frame.inv_view[2].xyz * view.z
        + frame.inv_view[3].xyz;
}

/// Computes true device depth from a view-space surface point.
fn sphere_view_depth(view: vec3f) -> f32 {
    let zw =
        frame.proj[0].zw * view.x
        + frame.proj[1].zw * view.y
        + frame.proj[2].zw * view.z
        + frame.proj[3].zw;

    return zw.x / zw.y;
}

/// Builds only the geometry required by every strip vertex.
fn sphere_geometry(
    atom: AtomRecord,
    vertex: u32,
) -> SphereGeometry {
    let world_center =
        atom_position(atom.entity_id);

    let center =
        sphere_view_position(
            world_center,
        );

    var radius = atom.radius;
    if visual_counts.visual_enabled != 0u {
        radius *= atom_visual_geometry(atom.entity_id).z * 4.0;
    }
    var half_size = vec2f(radius);

    if frame.projection_kind.x < 0.5 {
        half_size =
            sphere_quad_half_extent(
                center,
                radius,
            );
    }

    let view_position =
        center +
        vec3f(
            sphere_corner(vertex) *
                half_size,
            0.0,
        );

    return SphereGeometry(
        frame.proj *
            vec4f(view_position, 1.0),

        view_position.xy,

        vec4f(
            center,
            radius,
        ),

        world_center,
    );
}

/// Initializes outputs whose flat fields are supplied only by vertices 0/2.
fn sphere_vertex_output(
    geometry: SphereGeometry,
) -> SphereVsOut {
    var out: SphereVsOut;

    out.position =
        geometry.position;

    out.ray_xy =
        geometry.ray_xy;

    out.center_radius =
        vec4f(0.0);

    out.color =
        vec4f(0.0);

    out.previous_softness =
        vec4f(0.0);

    out.material =
        vec4f(0.0);

    out.entity_id =
        0u;

    return out;
}

/// Writes flat material data shared by opaque and transparent paths.
fn sphere_flat_material(
    atom: AtomRecord,
) -> vec4f {
    let response = atom_visual_response(atom.entity_id);
    let material = vec4f(
        response.x,
        response.y,
        representation.material.z,
        response.z,
    );
    return vec4f(
        varied_roughness(
            atom.entity_id,
            material.x,
        ),

        varied_roughness(
            atom.entity_id,
            SPHERE_CAP_ROUGHNESS,
        ),

        material_payload(
            material,
        ),

        1.0 / max(atom.radius * atom_visual_geometry(atom.entity_id).z * 4.0, 1.0e-6),
    );
}
