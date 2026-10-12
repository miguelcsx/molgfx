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

// Shape codes beyond the legacy pentagon/hexagon classes (see the packing in
// molgfx-render):
//   7  triangle (SNFG fucose-class deoxyhexose)
//   8  diamond (SNFG nonulosonates and uronates)
//   9  five-pointed star (SNFG pentoses); a non-convex union of six convex
//      pieces, so it is intersected piece by piece by `star_hit_piece`
// Codes 100..105 are the star's own convex pieces and never arrive from a
// record: 100 is the inner pentagon and 101..105 are the five tip triangles.
const POLYGON_TRIANGLE: u32 = 7u;
const POLYGON_DIAMOND: u32 = 8u;
const POLYGON_STAR: u32 = 9u;
const STAR_CORE: u32 = 100u;
const STAR_PIECES: u32 = 6u;

// Inner vertices of the pentagram, counter-clockwise, at radius
// 1 / golden_ratio^2 and angles 126, 198, 270, 342 and 54 degrees.
const STAR_INNER: array<vec2f, 5> = array<vec2f, 5>(
    vec2f(-0.22451399, 0.30901699),
    vec2f(-0.36327126, -0.11803399),
    vec2f(0.0, -0.38196601),
    vec2f(0.36327126, -0.11803399),
    vec2f(0.22451399, 0.30901699),
);

const POLYGON_TRIANGLE_VERTICES: array<vec2f, 3> = array<vec2f, 3>(
    vec2f(0.0, 1.0),
    vec2f(-0.86602540, -0.5),
    vec2f(0.86602540, -0.5),
);

const POLYGON_DIAMOND_VERTICES: array<vec2f, 4> = array<vec2f, 4>(
    vec2f(0.0, 1.0),
    vec2f(-1.0, 0.0),
    vec2f(0.0, -1.0),
    vec2f(1.0, 0.0),
);

fn polygon_sides(shape: u32) -> u32 {
    if shape == POLYGON_TRIANGLE || shape > STAR_CORE {
        return 3u;
    }

    if shape == POLYGON_DIAMOND {
        return 4u;
    }

    if shape == 4u || shape == 5u || shape == STAR_CORE {
        return 5u;
    }

    return 6u;
}

/// Vertex of one tip triangle: inner(k), tip(k + 1), inner(k + 1).
fn star_tip_vertex(piece: u32, index: u32) -> vec2f {
    let k = piece - STAR_CORE - 1u;
    if index == 1u {
        return PENTAGON[(k + 1u) % 5u];
    }

    return STAR_INNER[(k + index / 2u) % 5u];
}

/// Returns a precomputed regular polygon vertex.
///
/// shape == 6 stretches the hexagon along local X exactly as before.
fn polygon_vertex(
    shape: u32,
    index: u32,
) -> vec2f {
    if shape == POLYGON_TRIANGLE {
        return POLYGON_TRIANGLE_VERTICES[index];
    }

    if shape == POLYGON_DIAMOND {
        return POLYGON_DIAMOND_VERTICES[index];
    }

    if shape == STAR_CORE {
        return STAR_INNER[index];
    }

    if shape > STAR_CORE {
        return star_tip_vertex(shape, index);
    }

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

/// Nearest entry over the star's convex pieces; `.y` is the winning piece code.
fn star_hit_piece(
    origin: vec3f,
    direction: vec3f,
    half_size: vec3f,
) -> vec2f {
    var best = PRIMITIVE_INFINITY;
    var piece_code = f32(STAR_CORE);
    for (var piece = 0u; piece < STAR_PIECES; piece++) {
        let code = STAR_CORE + piece;
        let t = nearest_positive_interval(
            ray_polygon_interval(origin, direction, half_size, code)
        );
        if t > 0.0 && t < best {
            best = t;
            piece_code = f32(code);
        }
    }

    return vec2f(select(-1.0, best, best < PRIMITIVE_INFINITY), piece_code);
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

    var shape = in.metadata.w;
    var t = -1.0;
    if shape == POLYGON_STAR {
        let star = star_hit_piece(local.origin, local.direction, local.half_size);
        t = star.x;
        shape = u32(star.y);
    } else {
        t = nearest_positive_interval(
            ray_polygon_interval(
                local.origin,
                local.direction,
                local.half_size,
                shape,
            )
        );
    }

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
                shape,
            ),
        ),

        1.0,
        true,
    );
}
