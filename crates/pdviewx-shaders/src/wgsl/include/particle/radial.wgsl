// Sphere, billboard, circle, square and Gaussian particle hits.
//
// These shapes know their exact surface radius, so their normals come out
// normalized for free. Only the Gaussian, whose normal is a field gradient,
// pays for a true normalize.

fn particle_sphere_hit(
    in: PrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> PrimitiveHit {
    let radius =
        in.size.x * 0.5;

    let center =
        in.world_center - origin;

    // direction is normalized: quadratic coefficient a = 1.
    let t =
        nearest_positive_interval(
            ray_sphere_interval_precomputed(
                direction,
                center,
                radius * radius,
                1.0,
            )
        );

    if t <= 0.0 {
        return particle_miss();
    }

    let point =
        fma(
            direction,
            vec3f(t),
            origin,
        );

    return PrimitiveHit(
        t,
        (point - in.world_center) *
            particle_inverse_radius(radius),
        1.0,
        true,
    );
}

// -----------------------------------------------------------------------------
// Billboard circle / square
// -----------------------------------------------------------------------------

/// Intersects the local XY billboard plane without applying a shape test.
fn particle_plane_hit(
    in: PrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> ParticlePlaneHit {
    let frame =
        particle_local_frame(
            in,
            origin,
            direction,
        );

    let direction_z =
        frame.direction.z;

    if abs(direction_z) <= PARTICLE_RAY_EPSILON {
        return ParticlePlaneHit(
            vec2f(0.0),
            -1.0,
            0.0,
            false,
        );
    }

    let t =
        -frame.origin.z /
        direction_z;

    if t <= 0.0 {
        return ParticlePlaneHit(
            vec2f(0.0),
            -1.0,
            direction_z,
            false,
        );
    }

    return ParticlePlaneHit(
        fma(
            frame.direction.xy,
            vec2f(t),
            frame.origin.xy,
        ),
        t,
        direction_z,
        true,
    );
}

fn particle_billboard_normal(
    in: PrimitiveVsOut,
    direction_z: f32,
) -> vec3f {
    return particle_world_normal(
        in,
        vec3f(
            0.0,
            0.0,
            select(
                -1.0,
                1.0,
                direction_z < 0.0,
            ),
        ),
    );
}

fn particle_circle_hit(
    in: PrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> PrimitiveHit {
    let hit =
        particle_plane_hit(
            in,
            origin,
            direction,
        );

    if !hit.valid {
        return particle_miss();
    }

    let normalized =
        hit.point /
        max(
            in.size.xy * 0.5,
            vec2f(PARTICLE_SIZE_EPSILON),
        );

    if dot(normalized, normalized) > 1.0 {
        return particle_miss();
    }

    return PrimitiveHit(
        hit.t,
        particle_billboard_normal(
            in,
            hit.direction_z,
        ),
        1.0,
        true,
    );
}

fn particle_square_hit(
    in: PrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> PrimitiveHit {
    let hit =
        particle_plane_hit(
            in,
            origin,
            direction,
        );

    if !hit.valid {
        return particle_miss();
    }

    let normalized =
        hit.point /
        max(
            in.size.xy * 0.5,
            vec2f(PARTICLE_SIZE_EPSILON),
        );

    if any(
        abs(normalized) >
        vec2f(1.0)
    ) {
        return particle_miss();
    }

    return PrimitiveHit(
        hit.t,
        particle_billboard_normal(
            in,
            hit.direction_z,
        ),
        1.0,
        true,
    );
}

// -----------------------------------------------------------------------------
// Gaussian
// -----------------------------------------------------------------------------

fn particle_gaussian_hit(
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

    // Quaternion rotation preserves the normalized direction length.
    let t =
        -dot(
            frame.origin,
            frame.direction,
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

    let inverse_sigma =
        vec3f(1.0) /
        max(
            in.size *
                PARTICLE_GAUSSIAN_INV_SIGMA_SCALE,
            vec3f(PARTICLE_GAUSSIAN_EPSILON),
        );

    let normalized =
        point *
        inverse_sigma;

    let radius_sq =
        dot(
            normalized,
            normalized,
        );

    if radius_sq > 9.0 {
        return particle_miss();
    }

    let gradient =
        normalized *
        inverse_sigma;

    let gradient_sq =
        dot(
            gradient,
            gradient,
        );

    var local_normal: vec3f;

    if gradient_sq >
        PARTICLE_RAY_EPSILON {
        local_normal =
            gradient *
            inverseSqrt(gradient_sq);
    } else {
        local_normal =
            -frame.direction;
    }

    return PrimitiveHit(
        t,
        particle_world_normal(
            in,
            local_normal,
        ),
        exp(-0.5 * radius_sq),
        true,
    );
}
