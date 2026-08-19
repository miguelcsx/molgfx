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

/// Returns the conservative view-space sphere impostor half-size.
///
/// Takes squared center distance so callers can use dot(center, center)
/// instead of paying for length(center).
fn sphere_quad_half_size(
    center_distance_sq: f32,
    radius: f32,
) -> f32 {
    let radius_sq =
        radius * radius;

    let ratio_sq =
        clamp(
            radius_sq /
                max(
                    center_distance_sq,
                    SPHERE_MIN_DISTANCE_SQ,
                ),
            0.0,
            SPHERE_MAX_RATIO_SQ,
        );

    return radius *
        inverseSqrt(
            1.0 - ratio_sq
        );
}
