// Analytic ray-sphere intersection and surface resolution.
//
// The impostor is intersected in the fragment stage rather than tessellated,
// so the silhouette is exact at any zoom and an atom costs a point of
// instance data instead of a vertex ring. Clipped and unclipped resolution
// are separate, keeping the clip-plane work off the common path.

struct SphereRay {
    origin: vec3f,
    direction: vec3f,
}

/// Reconstructs one view-space ray for either camera projection.
fn sphere_ray(in: SphereVsOut) -> SphereRay {
    if frame.projection_kind.x > 0.5 {
        return SphereRay(
            vec3f(in.ray_xy, 0.0),
            vec3f(0.0, 0.0, -1.0),
        );
    }

    return SphereRay(
        vec3f(0.0),
        vec3f(in.ray_xy, in.center_radius.z),
    );
}

/// Solves the sphere quadratic once and retains the ray-center distance data
/// needed by transparent soft edges.
fn sphere_intersection(
    origin: vec3f,
    direction: vec3f,
    center_radius: vec4f,
) -> SphereIntersection {
    let center =
        center_radius.xyz - origin;

    let radius =
        center_radius.w;

    let direction_sq =
        dot(
            direction,
            direction,
        );

    if direction_sq <= SPHERE_RAY_EPSILON_SQ {
        return SphereIntersection(
            vec2f(1.0, 0.0),
            0.0,
            false,
        );
    }

    let center_dot_ray =
        dot(
            center,
            direction,
        );

    let center_sq =
        dot(
            center,
            center,
        );

    let discriminant =
        center_dot_ray *
            center_dot_ray -
        direction_sq *
            (
                center_sq -
                radius * radius
            );

    if discriminant < 0.0 {
        return SphereIntersection(
            vec2f(1.0, 0.0),
            0.0,
            false,
        );
    }

    let inverse_direction_sq =
        1.0 /
        direction_sq;

    let root =
        sqrt(discriminant);

    return SphereIntersection(
        vec2f(
            center_dot_ray - root,
            center_dot_ray + root,
        ) * inverse_direction_sq,

        max(
            center_sq -
                center_dot_ray *
                center_dot_ray *
                inverse_direction_sq,
            0.0,
        ),

        true,
    );
}

fn sphere_surface_miss() -> SphereSurface {
    return SphereSurface(
        vec3f(0.0),
        vec3f(0.0),
        0.0,
        false,
        false,
    );
}

/// Resolves the common unclipped sphere path with no clipping machinery.
fn sphere_surface_unclipped(
    in: SphereVsOut,
) -> SphereSurface {
    let ray =
        sphere_ray(in);

    let intersection =
        sphere_intersection(
            ray.origin,
            ray.direction,
            in.center_radius,
        );

    if !intersection.valid {
        return sphere_surface_miss();
    }

    let t =
        nearest_positive_interval(
            intersection.interval,
        );

    if t <= 0.0 {
        return sphere_surface_miss();
    }

    let hit =
        ray.origin + ray.direction * t;

    // An analytic sphere hit lies exactly radius units from its center.
    // Multiplying by the precomputed reciprocal avoids normalize().
    let normal =
        (
            hit -
            in.center_radius.xyz
        ) * in.material.w;

    return SphereSurface(
        hit,
        normal,
        intersection.perpendicular_sq,
        false,
        true,
    );
}

/// Resolves sphere + representation clipping only for clipped pipelines.
fn sphere_surface_clipped(
    in: SphereVsOut,
) -> SphereSurface {
    let ray =
        sphere_ray(in);

    let intersection =
        sphere_intersection(
            ray.origin,
            ray.direction,
            in.center_radius,
        );

    if !intersection.valid {
        return sphere_surface_miss();
    }

    let resolved =
        representation_primitive_hit(
            ray.origin,
            ray.direction,
            intersection.interval,
            frame.inv_view,
        );

    if !resolved.valid {
        return sphere_surface_miss();
    }

    let hit =
        ray.origin + ray.direction *
        resolved.t;

    var normal: vec3f;

    if resolved.cap {
        normal =
            primitive_view_normal(
                resolved,
                vec3f(0.0),
                frame.view,
            );
    } else {
        normal =
            (
                hit -
                in.center_radius.xyz
            ) * in.material.w;
    }

    return SphereSurface(
        hit,
        normal,
        intersection.perpendicular_sq,
        resolved.cap,
        true,
    );
}

/// Selects clip-cap material only inside clipped pipelines.
fn sphere_clipped_material(
    in: SphereVsOut,
    cap: bool,
) -> SphereMaterial {
    if cap {
        return SphereMaterial(
            vec4f(
                in.color.rgb *
                    SPHERE_CAP_TINT,
                SPHERE_CAP_MATERIAL,
            ),
            in.material.y,
        );
    }

    return SphereMaterial(
        vec4f(
            in.color.rgb,
            in.material.z,
        ),
        in.material.x,
    );
}
