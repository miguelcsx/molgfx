// Cylinder and spherocylinder particle hits.
//
// Both are solved from the same radial interval, with the spherocylinder
// replacing the flat axial clip with two spherical caps, so the two shapes
// share one solve rather than carrying two.

fn particle_cylinder_local_normal(
    point: vec3f,
    half_length: f32,
    inverse_radius: f32,
) -> vec3f {
    if abs(
        abs(point.z) -
        half_length
    ) <= 1.0e-4 {
        return vec3f(
            0.0,
            0.0,
            select(
                -1.0,
                1.0,
                point.z >= 0.0,
            ),
        );
    }

    // Exact cylinder side hit: |point.xy| == radius.
    return vec3f(
        point.xy *
            inverse_radius,
        0.0,
    );
}

fn particle_cylinder_hit(
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

    let radius =
        in.size.x * 0.5;

    let half_length =
        in.size.z * 0.5;

    let t =
        nearest_positive_interval(
            ray_cylinder_interval(
                frame.origin,
                frame.direction,
                radius,
                half_length,
            )
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
            particle_cylinder_local_normal(
                point,
                half_length,
                particle_inverse_radius(radius),
            ),
        ),
        1.0,
        true,
    );
}

// -----------------------------------------------------------------------------
// Spherocylinder
// -----------------------------------------------------------------------------

fn particle_spherocylinder_hit(
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

    let radius =
        in.size.x * 0.5;

    let half_segment =
        max(
            in.size.z * 0.5 -
                radius,
            0.0,
        );

    let endpoint =
        vec3f(
            0.0,
            0.0,
            half_segment,
        );

    let t =
        nearest_positive_interval(
            ray_capsule_interval(
                frame.direction,
                -endpoint -
                    frame.origin,
                endpoint -
                    frame.origin,
                radius,
            )
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

    let axis_point =
        vec3f(
            0.0,
            0.0,
            clamp(
                point.z,
                -half_segment,
                half_segment,
            ),
        );

    return PrimitiveHit(
        t,
        particle_world_normal(
            in,
            (point - axis_point) *
                particle_inverse_radius(radius),
        ),
        1.0,
        true,
    );
}
