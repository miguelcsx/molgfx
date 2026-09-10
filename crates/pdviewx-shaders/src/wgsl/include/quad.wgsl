// Six-vertex triangle-list impostor synthesis.
//
// Vertex order:
//   2 ---- 3
//   |    / |
//   |  /   |
//   0 ---- 1
//
// Triangle order: 0, 1, 2 and 2, 1, 3.

const SPHERE_MIN_DISTANCE_SQ: f32 = 1.0e-8;
const SPHERE_MAX_RATIO_SQ: f32 = 0.9801; // 0.99²

/// Returns triangle-list coordinates in [0, 1].
fn quad_uv(index: u32) -> vec2f {
    let corners = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
        vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
    );
    return corners[min(index, 5u)];
}

/// Returns triangle-list coordinates in [-1, 1].
fn quad_corner(index: u32) -> vec2f {
    return fma(
        quad_uv(index),
        vec2f(2.0),
        vec2f(-1.0),
    );
}

/// Returns exact axis-aligned view-plane extents of a projected sphere.
///
/// A perspective sphere silhouette is shifted away from the projection axis.
/// A symmetric bound derived only from center distance clips that shifted
/// silhouette for off-axis atoms, exposing the proxy quad as flat cuts. These
/// tangent-cone extrema include both the shift and the per-axis span.
fn sphere_quad_half_extent(
    center: vec3f,
    radius: f32,
) -> vec2f {
    let depth = max(-center.z, SPHERE_MIN_DISTANCE_SQ);
    let depth_sq = depth * depth;
    let radius_sq = min(radius * radius, depth_sq * SPHERE_MAX_RATIO_SQ);
    let denominator = max(depth_sq - radius_sq, SPHERE_MIN_DISTANCE_SQ);
    let lateral_root = sqrt(max(
        center.xy * center.xy + vec2f(denominator),
        vec2f(0.0),
    ));
    let shift = abs(center.xy) * radius_sq / denominator;
    let span = depth * sqrt(radius_sq) * lateral_root / denominator;
    return shift + span;
}
