// Exact analytic intersections shared by portable BVH and hardware AABB
// candidate traversal. Acceleration changes candidate discovery, not shape.

fn quality_sphere_distance(
    origin: vec3f,
    direction: vec3f,
    center: vec3f,
    radius: f32,
) -> f32 {
    let relative = origin - center;
    let b = dot(relative, direction);
    let c = dot(relative, relative) - radius * radius;
    let discriminant = b * b - c;
    return select(-1.0, -b - sqrt(discriminant), discriminant > 0.0);
}

fn quality_capsule_distance(
    origin: vec3f,
    direction: vec3f,
    endpoint_a: vec3f,
    endpoint_b: vec3f,
    radius: f32,
) -> f32 {
    return ray_capsule(
        direction,
        endpoint_a - origin,
        endpoint_b - origin,
        abs(radius),
    );
}
