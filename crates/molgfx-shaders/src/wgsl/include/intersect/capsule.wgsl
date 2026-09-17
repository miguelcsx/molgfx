// Ray-capsule intersection built from the cylinder side and two caps.
//
// One set of scalar products serves the side and both caps, so a bond
// impostor costs a single dot-product block rather than three independent
// primitive solves.

// -----------------------------------------------------------------------------
// Capsule
// -----------------------------------------------------------------------------

fn span_is_on_segment(
    span: f32,
    axis_len_sq: f32,
) -> bool {
    return span >= 0.0
        && span <= axis_len_sq;
}

/// Returns the sphere interval for endpoint A or B using only cached terms.
///
/// For B = A + axis:
///   dot(dir, B) = dot(dir, A) + dot(dir, axis)
///   |B|²-r²     = |A|²-r² + 2 dot(A,axis) + |axis|²
fn capsule_endpoint_interval(
    terms: CapsuleTerms,
    endpoint_b: bool,
) -> vec2f {
    let dir_center =
        select(
            terms.dir_a,
            terms.dir_a +
                terms.axis_dir,
            endpoint_b,
        );

    let center_sq_minus_radius_sq =
        select(
            terms.a_c,
            terms.a_c +
                2.0 * terms.axis_a +
                terms.axis_len_sq,
            endpoint_b,
        );

    return sphere_interval_from_terms(
        terms.dir_sq,
        dir_center,
        center_sq_minus_radius_sq,
    );
}

fn capsule_caps_interval(
    terms: CapsuleTerms,
) -> vec2f {
    return interval_union(
        capsule_endpoint_interval(
            terms,
            false,
        ),
        capsule_endpoint_interval(
            terms,
            true,
        ),
    );
}

/// Computes the infinite-cylinder side intersection.
///
/// Span coordinates remain multiplied by axis_len_sq so no projection
/// divisions are required.
fn capsule_side_interval(
    terms: CapsuleTerms,
    qa: f32,
) -> CapsuleSideInterval {
    let half_b =
        -terms.dir_a *
            terms.axis_len_sq +
        terms.axis_dir *
            terms.axis_a;

    let c =
        terms.a_c *
            terms.axis_len_sq -
        terms.axis_a *
            terms.axis_a;

    let interval =
        quadratic_interval(
            qa,
            half_b,
            c,
        );

    if interval_is_empty(interval) {
        return CapsuleSideInterval(
            interval,
            vec2f(0.0),
        );
    }

    return CapsuleSideInterval(
        interval,

        interval *
            terms.axis_dir -
        vec2f(terms.axis_a),
    );
}

/// Resolves finite-cylinder side roots with only the required spherical cap.
fn capsule_interval_from_side(
    terms: CapsuleTerms,
    side: CapsuleSideInterval,
) -> vec2f {
    let near_span =
        side.span_numerator.x;

    let far_span =
        side.span_numerator.y;

    let near_inside =
        span_is_on_segment(
            near_span,
            terms.axis_len_sq,
        );

    let far_inside =
        span_is_on_segment(
            far_span,
            terms.axis_len_sq,
        );

    if near_inside && far_inside {
        return side.interval;
    }

    if near_inside {
        return interval_union(
            vec2f(
                side.interval.x
            ),

            capsule_endpoint_interval(
                terms,
                far_span >
                    terms.axis_len_sq,
            ),
        );
    }

    if far_inside {
        return interval_union(
            capsule_endpoint_interval(
                terms,
                near_span >
                    terms.axis_len_sq,
            ),

            vec2f(
                side.interval.y
            ),
        );
    }

    let maximum_span =
        max(
            near_span,
            far_span,
        );

    if maximum_span < 0.0 {
        return capsule_endpoint_interval(
            terms,
            false,
        );
    }

    let minimum_span =
        min(
            near_span,
            far_span,
        );

    if minimum_span >
        terms.axis_len_sq {
        return capsule_endpoint_interval(
            terms,
            true,
        );
    }

    return capsule_caps_interval(
        terms
    );
}

/// Full line interval occupied by a convex capsule.
fn ray_capsule_interval(
    dir: vec3f,
    a: vec3f,
    b: vec3f,
    radius: f32,
) -> vec2f {
    let axis =
        b - a;

    let axis_len_sq =
        dot(axis, axis);

    if axis_len_sq <
        INTERSECT_AXIS_EPSILON {
        return ray_sphere_interval(
            dir,
            a,
            radius,
        );
    }

    let dir_sq =
        dot(dir, dir);

    if dir_sq <= 0.0 {
        return miss_interval();
    }

    let radius_sq =
        radius * radius;

    let axis_dir =
        dot(axis, dir);

    let axis_a =
        dot(axis, a);

    let dir_a =
        dot(dir, a);

    let terms =
        CapsuleTerms(
            dir_sq,
            axis_len_sq,
            axis_dir,
            axis_a,
            dir_a,

            dot(a, a) -
                radius_sq,
        );

    // Perpendicular quadratic coefficient multiplied by axis_len_sq.
    let qa =
        dir_sq *
            axis_len_sq -
        axis_dir *
            axis_dir;

    // Near-parallel rays use the convex hull of the endpoint sphere
    // intervals. The cylinder fills the span between them.
    if qa <=
        INTERSECT_AXIS_EPSILON *
        axis_len_sq {
        return capsule_caps_interval(
            terms
        );
    }

    let side =
        capsule_side_interval(
            terms,
            qa,
        );

    if interval_is_empty(
        side.interval
    ) {
        return miss_interval();
    }

    return capsule_interval_from_side(
        terms,
        side,
    );
}

/// Nearest positive capsule intersection, or -1 on miss.
fn ray_capsule(
    dir: vec3f,
    a: vec3f,
    b: vec3f,
    radius: f32,
) -> f32 {
    return nearest_positive_interval(
        ray_capsule_interval(
            dir,
            a,
            b,
            radius,
        )
    );
}
