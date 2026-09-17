// Analytic ellipsoid intersection.
//
// The ray is solved against the symmetric inverse tensor directly, so an
// ellipsoid never builds a temporary matrix and never tessellates: the
// silhouette stays exact at any zoom for the cost of one quadratic.

/// Multiplies the symmetric inverse ellipsoid tensor without constructing
/// a temporary matrix.
fn inverse_tensor_apply(
    primary: vec4f,
    cross_terms: vec4f,
    value: vec3f,
) -> vec3f {
    return vec3f(
        primary.x * value.x
            + primary.w * value.y
            + cross_terms.x * value.z,

        primary.w * value.x
            + primary.y * value.y
            + cross_terms.y * value.z,

        cross_terms.x * value.x
            + cross_terms.y * value.y
            + primary.z * value.z,
    );
}

fn ellipsoid_hit(
    in: PrimitiveVsOut,
    ray: PrimitiveRay,
) -> PrimitiveHit {
    let offset =
        ray.origin -
        in.world_center;

    let inverse_direction =
        inverse_tensor_apply(
            in.inverse_primary,
            in.inverse_cross,
            ray.direction,
        );

    let inverse_offset =
        inverse_tensor_apply(
            in.inverse_primary,
            in.inverse_cross,
            offset,
        );

    let a =
        dot(
            ray.direction,
            inverse_direction,
        );

    let half_b =
        dot(
            offset,
            inverse_direction,
        );

    let c =
        dot(
            offset,
            inverse_offset,
        ) - 1.0;

    let discriminant =
        half_b * half_b -
        a * c;

    if a <= PRIMITIVE_QUADRATIC_EPSILON
        || discriminant < 0.0 {
        return primitive_miss();
    }

    let root =
        sqrt(discriminant);

    let inv_a =
        1.0 / a;

    let interval =
        vec2f(
            (-half_b - root) * inv_a,
            (-half_b + root) * inv_a,
        );

    let t =
        nearest_positive_interval(
            interval,
        );

    if t <= 0.0 {
        return primitive_miss();
    }

    let point =
        fma(
            ray.direction,
            vec3f(t),
            ray.origin,
        );

    let normal =
        normalize(
            inverse_tensor_apply(
                in.inverse_primary,
                in.inverse_cross,
                point - in.world_center,
            )
        );

    return PrimitiveHit(
        t,
        normal,
        1.0,
        true,
    );
}
