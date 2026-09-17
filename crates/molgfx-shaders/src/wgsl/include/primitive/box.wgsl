// Analytic oriented-box intersection.
//
// Slab intervals are accumulated per axis in the box's own frame, so the test
// is three interval clips and no matrix inverse. The normal is recovered from
// which slab produced the entry point rather than from a stored face index.

fn ray_axis_interval(
    origin: f32,
    direction: f32,
    half_extent: f32,
) -> vec2f {
    if abs(direction) <=
        PRIMITIVE_DIRECTION_EPSILON {
        if abs(origin) > half_extent {
            return vec2f(1.0, 0.0);
        }

        return vec2f(
            -PRIMITIVE_INFINITY,
            PRIMITIVE_INFINITY,
        );
    }

    let inverse_direction =
        1.0 / direction;

    let first =
        (-half_extent - origin) *
        inverse_direction;

    let second =
        (half_extent - origin) *
        inverse_direction;

    return vec2f(
        min(first, second),
        max(first, second),
    );
}

fn ray_box_interval(
    origin: vec3f,
    direction: vec3f,
    half_size: vec3f,
) -> vec2f {
    let x =
        ray_axis_interval(
            origin.x,
            direction.x,
            half_size.x,
        );

    let y =
        ray_axis_interval(
            origin.y,
            direction.y,
            half_size.y,
        );

    let z =
        ray_axis_interval(
            origin.z,
            direction.z,
            half_size.z,
        );

    let near =
        max(
            max(x.x, y.x),
            z.x,
        );

    let far =
        min(
            min(x.y, y.y),
            z.y,
        );

    if near > far || far <= 0.0 {
        return vec2f(1.0, 0.0);
    }

    return vec2f(near, far);
}

fn box_normal(
    point: vec3f,
    half_size: vec3f,
) -> vec3f {
    let distance =
        abs(
            abs(point) -
            half_size
        );

    if distance.x <= distance.y
        && distance.x <= distance.z {
        return vec3f(
            select(
                -1.0,
                1.0,
                point.x >= 0.0,
            ),
            0.0,
            0.0,
        );
    }

    if distance.y <= distance.z {
        return vec3f(
            0.0,
            select(
                -1.0,
                1.0,
                point.y >= 0.0,
            ),
            0.0,
        );
    }

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

fn box_hit(
    in: PrimitiveVsOut,
    ray: PrimitiveRay,
) -> PrimitiveHit {
    let local =
        oriented_ray(
            in,
            ray,
        );

    let t =
        nearest_positive_interval(
            ray_box_interval(
                local.origin,
                local.direction,
                local.half_size,
            )
        );

    if t <= 0.0 {
        return primitive_miss();
    }

    let local_point =
        fma(
            local.direction,
            vec3f(t),
            local.origin,
        );

    return PrimitiveHit(
        t,

        rotate_vector(
            local.orientation,
            box_normal(
                local_point,
                local.half_size,
            ),
        ),

        1.0,
        true,
    );
}
