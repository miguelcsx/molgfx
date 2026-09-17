// Bounded implicit superquadric intersections for depth-only shadows.

const SHADOW_SUPERQUADRIC_STEPS: u32 = 64u;
const SHADOW_SUPERQUADRIC_REFINEMENTS: u32 = 12u;
const SHADOW_SUPERQUADRIC_INV_STEPS: f32 = 1.0 / 64.0;
const SHADOW_SUPERQUADRIC_SIZE_EPSILON: f32 = 1.0e-5;

struct ShadowSuperquadricParams {
    inv_half_size: vec3f,
    xy_power: f32,
    z_power: f32,
    radial_power: f32,
}

fn shadow_superquadric_params(
    half_size: vec3f,
    exponents: vec2f,
) -> ShadowSuperquadricParams {
    let latitude = clamp(exponents.x, 0.1, 8.0);
    let longitude = clamp(exponents.y, 0.1, 8.0);
    let inverse_latitude = 1.0 / latitude;
    return ShadowSuperquadricParams(
        vec3f(1.0) /
            max(
                half_size,
                vec3f(SHADOW_SUPERQUADRIC_SIZE_EPSILON),
            ),
        2.0 / longitude,
        2.0 * inverse_latitude,
        longitude * inverse_latitude,
    );
}

fn shadow_superquadric_value(
    point: vec3f,
    params: ShadowSuperquadricParams,
) -> f32 {
    let normalized = abs(point) * params.inv_half_size;
    let xy = pow(
        normalized.xy,
        vec2f(params.xy_power),
    );
    return pow(xy.x + xy.y, params.radial_power)
        + pow(normalized.z, params.z_power)
        - 1.0;
}

fn shadow_superquadric_refine(
    ray: mat2x3f,
    low_t: f32,
    high_t: f32,
    low_value: f32,
    params: ShadowSuperquadricParams,
) -> f32 {
    var low = low_t;
    var high = high_t;
    let low_negative = low_value < 0.0;
    for (
        var refinement = 0u;
        refinement < SHADOW_SUPERQUADRIC_REFINEMENTS;
        refinement++
    ) {
        let middle = (low + high) * 0.5;
        let value = shadow_superquadric_value(
            fma(ray[1], vec3f(middle), ray[0]),
            params,
        );
        if (value < 0.0) == low_negative {
            low = middle;
        } else {
            high = middle;
        }
    }
    return (low + high) * 0.5;
}

fn shadow_superquadric_find(
    ray: mat2x3f,
    interval: vec2f,
    params: ShadowSuperquadricParams,
) -> f32 {
    if interval.x > interval.y || interval.y <= 0.0 {
        return -1.0;
    }
    let start_t = max(interval.x, 0.0);
    let step_t =
        (interval.y - start_t) *
        SHADOW_SUPERQUADRIC_INV_STEPS;
    if step_t <= 0.0 {
        return -1.0;
    }
    let step_point = ray[1] * step_t;
    var previous_t = start_t;
    var point = fma(ray[1], vec3f(start_t), ray[0]);
    var previous_value =
        shadow_superquadric_value(point, params);
    if previous_value == 0.0 {
        return start_t;
    }
    for (var step = 0u; step < SHADOW_SUPERQUADRIC_STEPS; step++) {
        let current_t = previous_t + step_t;
        point += step_point;
        let current_value =
            shadow_superquadric_value(point, params);
        if current_value == 0.0 {
            return current_t;
        }
        if (previous_value < 0.0) != (current_value < 0.0) {
            return shadow_superquadric_refine(
                ray,
                previous_t,
                current_t,
                previous_value,
                params,
            );
        }
        previous_t = current_t;
        previous_value = current_value;
    }
    return -1.0;
}

fn shadow_superquadric_t(
    in: ShadowPrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> f32 {
    let ray = shadow_local_ray(in, origin, direction);
    let half_size = in.size * 0.5;
    let interval = shadow_box_interval(
        ray[0],
        ray[1],
        half_size,
    );
    let params = shadow_superquadric_params(
        half_size,
        in.inverse_primary.xy,
    );
    return shadow_superquadric_find(ray, interval, params);
}
