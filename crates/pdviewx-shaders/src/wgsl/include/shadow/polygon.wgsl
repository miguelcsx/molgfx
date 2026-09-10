// Exact regular-polygon prism intersections for carbohydrate shadows.

const SHADOW_PENTAGON: array<vec2f, 5> = array<vec2f, 5>(
    vec2f(0.0, 1.0),
    vec2f(-0.95105652, 0.30901699),
    vec2f(-0.58778525, -0.80901699),
    vec2f(0.58778525, -0.80901699),
    vec2f(0.95105652, 0.30901699),
);

const SHADOW_HEXAGON: array<vec2f, 6> = array<vec2f, 6>(
    vec2f(0.0, 1.0),
    vec2f(-0.86602540, 0.5),
    vec2f(-0.86602540, -0.5),
    vec2f(0.0, -1.0),
    vec2f(0.86602540, -0.5),
    vec2f(0.86602540, 0.5),
);

fn shadow_cross_2d(left: vec2f, right: vec2f) -> f32 {
    return left.x * right.y - left.y * right.x;
}

fn shadow_polygon_vertex(shape: u32, index: u32) -> vec2f {
    if SHADOW_PRIMITIVE_KIND ==
        SHADOW_KIND_POLYGON_PENTAGON {
        return SHADOW_PENTAGON[index];
    }
    let vertex = SHADOW_HEXAGON[index];
    return select(
        vertex,
        vec2f(vertex.x * 1.18, vertex.y),
        shape == 6u,
    );
}

fn shadow_polygon_interval(
    origin: vec3f,
    direction: vec3f,
    half_size: vec3f,
    shape: u32,
) -> vec2f {
    let z = shadow_axis_interval(
        origin.z,
        direction.z,
        half_size.z,
    );
    if interval_is_empty(z) {
        return miss_interval();
    }
    let sides = select(
        6u,
        5u,
        SHADOW_PRIMITIVE_KIND ==
            SHADOW_KIND_POLYGON_PENTAGON,
    );
    var near = z.x;
    var far = z.y;
    var current =
        shadow_polygon_vertex(shape, 0u) *
        half_size.xy;
    for (var index = 0u; index < sides; index++) {
        let next_index = select(
            0u,
            index + 1u,
            index + 1u < sides,
        );
        let next =
            shadow_polygon_vertex(shape, next_index) *
            half_size.xy;
        let edge = next - current;
        let constant = shadow_cross_2d(
            edge,
            origin.xy - current,
        );
        let slope = shadow_cross_2d(edge, direction.xy);
        if abs(slope) <= SHADOW_DIRECTION_EPSILON {
            if constant < 0.0 {
                return miss_interval();
            }
        } else {
            let crossing = -constant / slope;
            if slope > 0.0 {
                near = max(near, crossing);
            } else {
                far = min(far, crossing);
            }
            if near > far {
                return miss_interval();
            }
        }
        current = next;
    }
    if far <= 0.0 {
        return miss_interval();
    }
    return vec2f(near, far);
}

fn shadow_polygon_t(
    in: ShadowPrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> f32 {
    let ray = shadow_local_ray(in, origin, direction);
    return nearest_positive_interval(
        shadow_polygon_interval(
            ray[0],
            ray[1],
            in.size * 0.5,
            in.shape,
        )
    );
}
