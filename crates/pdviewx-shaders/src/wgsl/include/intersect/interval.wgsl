// Ray-interval algebra shared by every analytic primitive.
//
// A hit is a sorted [near, far] pair and a miss is near > far, so union and
// intersection are branch-free min/max work. Compound shapes are then built
// by combining intervals instead of by writing a bespoke solver per shape.

const INTERSECT_INFINITY: f32 = 1.0e30;
const INTERSECT_RADIAL_EPSILON: f32 = 1.0e-7;
const INTERSECT_AXIS_EPSILON: f32 = 1.0e-12;

struct CapsuleTerms {
    dir_sq: f32,
    axis_len_sq: f32,
    axis_dir: f32,
    axis_a: f32,
    dir_a: f32,
    a_c: f32,
}

struct CapsuleSideInterval {
    interval: vec2f,
    span_numerator: vec2f,
}

fn miss_interval() -> vec2f {
    return vec2f(1.0, 0.0);
}

fn infinite_interval() -> vec2f {
    return vec2f(
        -INTERSECT_INFINITY,
        INTERSECT_INFINITY,
    );
}

fn interval_is_empty(interval: vec2f) -> bool {
    return interval.x > interval.y;
}

/// Returns the sorted roots of a quadratic with positive coefficient `a`.
fn quadratic_interval(
    a: f32,
    half_b: f32,
    c: f32,
) -> vec2f {
    if a <= 0.0 {
        return miss_interval();
    }

    let discriminant =
        half_b * half_b -
        a * c;

    if discriminant < 0.0 {
        return miss_interval();
    }

    let root =
        sqrt(discriminant);

    let inv_a =
        1.0 / a;

    return vec2f(
        (-half_b - root) * inv_a,
        (-half_b + root) * inv_a,
    );
}

/// Returns the convex hull of two intervals, preserving misses.
///
/// A single vec2 cannot represent two disjoint intervals; capsule callers
/// intentionally use the hull because a capsule itself is convex.
fn interval_union(
    left: vec2f,
    right: vec2f,
) -> vec2f {
    if interval_is_empty(left) {
        return right;
    }

    if interval_is_empty(right) {
        return left;
    }

    return vec2f(
        min(left.x, right.x),
        max(left.y, right.y),
    );
}

fn interval_intersection(
    left: vec2f,
    right: vec2f,
) -> vec2f {
    return vec2f(
        max(left.x, right.x),
        min(left.y, right.y),
    );
}

/// Returns the nearest positive endpoint, or -1 on miss.
fn nearest_positive_interval(
    interval: vec2f,
) -> f32 {
    if interval_is_empty(interval)
        || interval.y <= 0.0 {
        return -1.0;
    }

    return select(
        interval.y,
        interval.x,
        interval.x > 0.0,
    );
}

/// Compatibility alias for existing callers.
fn nearest_positive(interval: vec2f) -> f32 {
    return nearest_positive_interval(interval);
}
