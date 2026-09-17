// Ray-infinite-cylinder intersection, split into radial and axial intervals.
//
// Keeping the two intervals separate is what lets a capsule reuse the radial
// solve while replacing the axial clip with its caps, so the shared
// ray-primitive machinery is written once.

// -----------------------------------------------------------------------------
// Cylinder
// -----------------------------------------------------------------------------

fn cylinder_radial_interval(
    origin: vec2f,
    direction: vec2f,
    radius_sq: f32,
) -> vec2f {
    let a =
        dot(direction, direction);

    let c =
        dot(origin, origin) -
        radius_sq;

    if a <= INTERSECT_RADIAL_EPSILON {
        return select(
            infinite_interval(),
            miss_interval(),
            c > 0.0,
        );
    }

    return quadratic_interval(
        a,
        dot(origin, direction),
        c,
    );
}

fn cylinder_axial_interval(
    origin: f32,
    direction: f32,
    half_length: f32,
) -> vec2f {
    if abs(direction) <=
        INTERSECT_RADIAL_EPSILON {
        return select(
            infinite_interval(),
            miss_interval(),
            abs(origin) > half_length,
        );
    }

    let inverse_direction =
        1.0 / direction;

    let roots =
        vec2f(
            -half_length - origin,
            half_length - origin,
        ) * inverse_direction;

    return vec2f(
        min(roots.x, roots.y),
        max(roots.x, roots.y),
    );
}

/// Full interval occupied by a finite Z-aligned cylinder.
fn ray_cylinder_interval(
    origin: vec3f,
    direction: vec3f,
    radius: f32,
    half_length: f32,
) -> vec2f {
    let radial =
        cylinder_radial_interval(
            origin.xy,
            direction.xy,
            radius * radius,
        );

    // Nothing positive can survive axial clipping if the radial interval is
    // already empty or entirely behind the ray origin.
    if interval_is_empty(radial)
        || radial.y <= 0.0 {
        return miss_interval();
    }

    let axial =
        cylinder_axial_interval(
            origin.z,
            direction.z,
            half_length,
        );

    if interval_is_empty(axial) {
        return miss_interval();
    }

    let interval =
        interval_intersection(
            radial,
            axial,
        );

    if interval_is_empty(interval)
        || interval.y <= 0.0 {
        return miss_interval();
    }

    return interval;
}
