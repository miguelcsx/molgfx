// Bounded implicit superquadric particle hits.
//
// A superquadric has no closed-form root, so the ray is sampled along its
// bounded span and refined only across a sign change. Cost is a fixed number
// of steps and refinements per covered pixel, bounded by construction, which
// keeps the worst case predictable inside a frame budget.

fn superquadric_params(
    half_size: vec3f,
    exponents: vec2f,
) -> SuperquadricParams {
    let latitude =
        clamp(
            exponents.x,
            0.1,
            8.0,
        );

    let longitude =
        clamp(
            exponents.y,
            0.1,
            8.0,
        );

    let inverse_latitude =
        1.0 / latitude;

    return SuperquadricParams(
        vec3f(1.0) /
        max(
            half_size,
            vec3f(PARTICLE_SIZE_EPSILON),
        ),
        2.0 / longitude,
        2.0 * inverse_latitude,
        longitude * inverse_latitude,
    );
}

fn superquadric_value(
    point: vec3f,
    params: SuperquadricParams,
) -> f32 {
    let normalized =
        abs(point) *
        params.inv_half_size;

    let xy =
        pow(
            normalized.xy,
            vec2f(params.xy_power),
        );

    return pow(
        xy.x + xy.y,
        params.radial_power,
    ) +
        pow(
            normalized.z,
            params.z_power,
        ) -
        1.0;
}

fn superquadric_refine_hit(
    frame: ParticleLocalFrame,
    low_t: f32,
    high_t: f32,
    low_value: f32,
    params: SuperquadricParams,
) -> f32 {
    var low =
        low_t;

    var high =
        high_t;

    let low_negative =
        low_value < 0.0;

    for (
        var refinement = 0u;
        refinement < SUPERQUADRIC_REFINEMENTS;
        refinement++
    ) {
        let middle =
            (low + high) * 0.5;

        let value =
            superquadric_value(
                fma(
                    frame.direction,
                    vec3f(middle),
                    frame.origin,
                ),
                params,
            );

        if (value < 0.0) ==
            low_negative {
            low =
                middle;
        } else {
            high =
                middle;
        }
    }

    return (low + high) * 0.5;
}

fn superquadric_find_hit(
    frame: ParticleLocalFrame,
    interval: vec2f,
    params: SuperquadricParams,
) -> f32 {
    if interval.x > interval.y ||
        interval.y <= 0.0 {
        return -1.0;
    }

    let start_t =
        max(
            interval.x,
            0.0,
        );

    let step_t =
        (
            interval.y -
            start_t
        ) * SUPERQUADRIC_INV_STEPS;

    if step_t <= 0.0 {
        return -1.0;
    }

    let step_point =
        frame.direction *
        step_t;

    var previous_t =
        start_t;

    var point =
        fma(
            frame.direction,
            vec3f(start_t),
            frame.origin,
        );

    var previous_value =
        superquadric_value(
            point,
            params,
        );

    if previous_value == 0.0 {
        return start_t;
    }

    for (
        var step = 0u;
        step < SUPERQUADRIC_STEPS;
        step++
    ) {
        let current_t =
            previous_t +
            step_t;

        point +=
            step_point;

        let current_value =
            superquadric_value(
                point,
                params,
            );

        if current_value == 0.0 {
            return current_t;
        }

        if (previous_value < 0.0) !=
            (current_value < 0.0) {
            return superquadric_refine_hit(
                frame,
                previous_t,
                current_t,
                previous_value,
                params,
            );
        }

        previous_t =
            current_t;

        previous_value =
            current_value;
    }

    return -1.0;
}

fn superquadric_local_normal(
    point: vec3f,
    params: SuperquadricParams,
) -> vec3f {
    let normalized =
        abs(point) *
        params.inv_half_size;

    let xy =
        pow(
            normalized.xy,
            vec2f(params.xy_power),
        );

    let radial_base =
        xy.x + xy.y;

    let radial_scale =
        pow(
            max(
                radial_base,
                SUPERQUADRIC_GRADIENT_EPSILON,
            ),
            params.radial_power - 1.0,
        );

    let safe_xy =
        max(
            normalized.xy,
            vec2f(
                SUPERQUADRIC_GRADIENT_EPSILON
            ),
        );

    let safe_z =
        max(
            normalized.z,
            SUPERQUADRIC_GRADIENT_EPSILON,
        );

    let gradient =
        vec3f(
            sign(point.x) *
                params.inv_half_size.x *
                radial_scale *
                xy.x /
                safe_xy.x,

            sign(point.y) *
                params.inv_half_size.y *
                radial_scale *
                xy.y /
                safe_xy.y,

            sign(point.z) *
                params.inv_half_size.z *
                pow(
                    normalized.z,
                    params.z_power,
                ) /
                safe_z,
        );

    return gradient *
        inverseSqrt(
            max(
                dot(gradient, gradient),
                SUPERQUADRIC_GRADIENT_EPSILON,
            )
        );
}

fn particle_superquadric_hit(
    in: PrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> PrimitiveHit {
    let frame =
        particle_local_frame(
            in,
            origin,
            direction,
        );

    let half_size =
        in.size * 0.5;

    let interval =
        ray_box_interval(
            frame.origin,
            frame.direction,
            half_size,
        );

    if interval.x > interval.y ||
        interval.y <= 0.0 {
        return particle_miss();
    }

    let params =
        superquadric_params(
            half_size,
            in.inverse_primary.xy,
        );

    let t =
        superquadric_find_hit(
            frame,
            interval,
            params,
        );

    if t <= 0.0 {
        return particle_miss();
    }

    let point =
        fma(
            frame.direction,
            vec3f(t),
            frame.origin,
        );

    return PrimitiveHit(
        t,
        particle_world_normal(
            in,
            superquadric_local_normal(
                point,
                params,
            ),
        ),
        1.0,
        true,
    );
}

// -----------------------------------------------------------------------------
// Pipeline-specialized dispatcher
// -----------------------------------------------------------------------------
