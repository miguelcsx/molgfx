// Ray-sphere intersection.
//
// The direction is normalized by the caller, so the quadratic drops its `a`
// term. A variant takes already-computed scalar products, letting the capsule
// solve its caps without recomputing what it has in registers.

fn sphere_interval_from_terms(
    dir_sq: f32,
    dir_center: f32,
    center_sq_minus_radius_sq: f32,
) -> vec2f {
    return quadratic_interval(
        dir_sq,
        -dir_center,
        center_sq_minus_radius_sq,
    );
}

fn ray_sphere_interval_precomputed(
    dir: vec3f,
    center: vec3f,
    radius_sq: f32,
    dir_sq: f32,
) -> vec2f {
    return sphere_interval_from_terms(
        dir_sq,
        dot(dir, center),
        dot(center, center) - radius_sq,
    );
}

/// Complete line interval occupied by a sphere.
fn ray_sphere_interval(
    dir: vec3f,
    center: vec3f,
    radius: f32,
) -> vec2f {
    let radius_sq =
        radius * radius;

    return sphere_interval_from_terms(
        dot(dir, dir),
        dot(dir, center),
        dot(center, center) - radius_sq,
    );
}

/// Nearest positive sphere intersection, or -1 on miss.
fn ray_sphere(
    dir: vec3f,
    center: vec3f,
    radius: f32,
) -> f32 {
    return nearest_positive_interval(
        ray_sphere_interval(
            dir,
            center,
            radius,
        )
    );
}
