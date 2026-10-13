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

const SHADOW_TRIANGLE: array<vec2f, 3> = array<vec2f, 3>(
    vec2f(0.0, 1.0),
    vec2f(-0.86602540, -0.5),
    vec2f(0.86602540, -0.5),
);

const SHADOW_DIAMOND: array<vec2f, 4> = array<vec2f, 4>(
    vec2f(0.0, 1.0),
    vec2f(-1.0, 0.0),
    vec2f(0.0, -1.0),
    vec2f(1.0, 0.0),
);

// Pentagram inner vertices; the same table the visible pass intersects.
const SHADOW_STAR_INNER: array<vec2f, 5> = array<vec2f, 5>(
    vec2f(-0.22451399, 0.30901699),
    vec2f(-0.36327126, -0.11803399),
    vec2f(0.0, -0.38196601),
    vec2f(0.36327126, -0.11803399),
    vec2f(0.22451399, 0.30901699),
);

// Record shape codes of the SNFG polygons; 100 is the star's inner pentagon
// and 101..105 its tip triangles, which never arrive from a record.
const SHADOW_SHAPE_TRIANGLE: u32 = 7u;
const SHADOW_SHAPE_DIAMOND: u32 = 8u;
const SHADOW_SHAPE_STAR: u32 = 9u;
const SHADOW_STAR_CORE: u32 = 100u;
const SHADOW_STAR_PIECES: u32 = 6u;

fn shadow_kind_is_polygon() -> bool {
    return SHADOW_PRIMITIVE_KIND == SHADOW_KIND_POLYGON_PENTAGON
        || SHADOW_PRIMITIVE_KIND == SHADOW_KIND_POLYGON_HEXAGON
        || SHADOW_PRIMITIVE_KIND == SHADOW_KIND_POLYGON_TRIANGLE
        || SHADOW_PRIMITIVE_KIND == SHADOW_KIND_POLYGON_DIAMOND
        || SHADOW_PRIMITIVE_KIND == SHADOW_KIND_POLYGON_STAR;
}

fn shadow_polygon_sides(shape: u32) -> u32 {
    if shape == SHADOW_SHAPE_TRIANGLE || shape > SHADOW_STAR_CORE {
        return 3u;
    }
    if shape == SHADOW_SHAPE_DIAMOND {
        return 4u;
    }
    if shape == 4u || shape == 5u || shape == SHADOW_STAR_CORE {
        return 5u;
    }
    return 6u;
}

fn shadow_polygon_vertex(shape: u32, index: u32) -> vec2f {
    if shape == SHADOW_SHAPE_TRIANGLE {
        return SHADOW_TRIANGLE[index];
    }
    if shape == SHADOW_SHAPE_DIAMOND {
        return SHADOW_DIAMOND[index];
    }
    if shape == SHADOW_STAR_CORE {
        return SHADOW_STAR_INNER[index];
    }
    if shape > SHADOW_STAR_CORE {
        let tip = shape - SHADOW_STAR_CORE - 1u;
        if index == 1u {
            return SHADOW_PENTAGON[(tip + 1u) % 5u];
        }
        return SHADOW_STAR_INNER[(tip + index / 2u) % 5u];
    }
    if shape == 4u || shape == 5u {
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
    let sides = shadow_polygon_sides(shape);
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
    let half_size = in.size * 0.5;
    if in.shape != SHADOW_SHAPE_STAR {
        return nearest_positive_interval(
            shadow_polygon_interval(ray[0], ray[1], half_size, in.shape)
        );
    }
    // The star is a union of convex pieces: the nearest entry wins.
    var best = -1.0;
    for (var piece = 0u; piece < SHADOW_STAR_PIECES; piece++) {
        let t = nearest_positive_interval(
            shadow_polygon_interval(
                ray[0],
                ray[1],
                half_size,
                SHADOW_STAR_CORE + piece,
            )
        );
        if t > 0.0 && (best < 0.0 || t < best) {
            best = t;
        }
    }
    return best;
}
