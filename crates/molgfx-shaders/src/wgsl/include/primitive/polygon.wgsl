// Analytic regular-polygon prism intersection.
//
// Carbohydrate symbols are regular polygons, so the side planes are generated
// from the vertex count instead of stored per shape. The ray is clipped
// against those planes in the polygon's own 2D frame, giving O(sides) work
// per fragment with no vertex buffer behind it.

fn cross_2d(
    left: vec2f,
    right: vec2f,
) -> f32 {
    return left.x * right.y -
        left.y * right.x;
}

fn polygon_sides(shape: u32) -> u32 {
    if shape == 4u || shape == 5u {
        return 5u;
    }

    return 6u;
}

/// Returns a precomputed regular polygon vertex.
///
/// shape == 6 stretches the hexagon along local X exactly as before.
fn polygon_vertex(
    shape: u32,
    index: u32,
) -> vec2f {
    if shape == 4u || shape == 5u {
        return PENTAGON[index];
    }

    let vertex =
        HEXAGON[index];

    if shape == 6u {
        return vec2f(
            vertex.x * 1.18,
            vertex.y,
        );
    }

    return vertex;
}

fn ray_polygon_interval(
    origin: vec3f,
    direction: vec3f,
    half_size: vec3f,
    shape: u32,
) -> vec2f {
    let z =
        ray_axis_interval(
            origin.z,
            direction.z,
            half_size.z,
        );

    if z.x > z.y {
        return vec2f(1.0, 0.0);
    }

    let sides =
        polygon_sides(shape);

    var near =
        z.x;

    var far =
        z.y;

    var current =
        polygon_vertex(
            shape,
            0u,
        ) * half_size.xy;

    for (
        var index = 0u;
        index < sides;
        index++
    ) {
        let next_index =
            select(
                0u,
                index + 1u,
                index + 1u < sides,
            );

        let next =
            polygon_vertex(
                shape,
                next_index,
            ) * half_size.xy;

        let edge =
            next - current;

        let constant =
            cross_2d(
                edge,
                origin.xy - current,
            );

        let slope =
            cross_2d(
                edge,
                direction.xy,
            );

        if abs(slope) <=
            PRIMITIVE_DIRECTION_EPSILON {
            if constant < 0.0 {
                return vec2f(1.0, 0.0);
            }
        } else {
            let crossing =
                -constant / slope;

            if slope > 0.0 {
                near =
                    max(
                        near,
                        crossing,
                    );
            } else {
                far =
                    min(
                        far,
                        crossing,
                    );
            }

            if near > far {
                return vec2f(1.0, 0.0);
            }
        }

        current =
            next;
    }

    if far <= 0.0 {
        return vec2f(1.0, 0.0);
    }

    return vec2f(near, far);
}

fn polygon_normal(
    point: vec3f,
    half_size: vec3f,
    shape: u32,
) -> vec3f {
    var best =
        abs(
            abs(point.z) -
            half_size.z
        );

    var normal =
        vec3f(
            0.0,
            0.0,
            select(
                -1.0,
                1.0,
                point.z >= 0.0,
            ),
        );

    let sides =
        polygon_sides(shape);

    var current =
        polygon_vertex(
            shape,
            0u,
        ) * half_size.xy;

    for (
        var index = 0u;
        index < sides;
        index++
    ) {
        let next_index =
            select(
                0u,
                index + 1u,
                index + 1u < sides,
            );

        let next =
            polygon_vertex(
                shape,
                next_index,
            ) * half_size.xy;

        let edge =
            next - current;

        let outward =
            normalize(
                vec2f(
                    edge.y,
                    -edge.x,
                )
            );

        let distance =
            abs(
                dot(
                    outward,
                    point.xy - current,
                )
            );

        if distance < best {
            best =
                distance;

            normal =
                vec3f(
                    outward,
                    0.0,
                );
        }

        current =
            next;
    }

    return normal;
}

fn polygon_hit(
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
            ray_polygon_interval(
                local.origin,
                local.direction,
                local.half_size,
                in.metadata.w,
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
            polygon_normal(
                local_point,
                local.half_size,
                in.metadata.w,
            ),
        ),

        1.0,
        true,
    );
}
