// Caller-authored primitive shadow casters.
//
// The primitive family is fixed at pipeline creation rather than branched per
// fragment, so a shadow draw resolves exactly one shape. Caller quaternions
// arrive normalized, so rotating a ray preserves its length without a
// renormalize in the inner loop.

struct ShadowPrimitiveVsOut {
    @builtin(position) position: vec4f,

    @location(0) @interpolate(linear) light_xy: vec2f,

    @location(1) @interpolate(flat, first) world_center: vec3f,
    @location(2) @interpolate(flat, first) orientation: vec4f,
    @location(3) @interpolate(flat, first) size: vec3f,
    @location(4) @interpolate(flat, first) inverse_primary: vec4f,
    @location(5) @interpolate(flat, first) inverse_cross: vec4f,
}

@vertex
fn vs_shadow_primitive(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> ShadowPrimitiveVsOut {
    let center_radius =
        shadow_primitives[instance]
            .center_radius;

    let center =
        shadow_view_position(
            center_radius.xyz
        );

    let bound =
        abs(center_radius.w);

    let light_xy =
        center.xy
        + quad_corner(vertex) * bound;

    var out: ShadowPrimitiveVsOut;

    out.position =
        shadow_clip(
            vec3f(
                light_xy,
                center.z,
            )
        );

    out.light_xy =
        light_xy;

    out.world_center =
        vec3f(0.0);

    out.orientation =
        vec4f(0.0);

    out.size =
        vec3f(0.0);

    out.inverse_primary =
        vec4f(0.0);

    out.inverse_cross =
        vec4f(0.0);

    // Only vertices 0 and 3 provide flat data for the independent triangles.
    if shadow_flat_source(vertex) {
        out.world_center =
            center_radius.xyz;

        if SHADOW_PRIMITIVE_KIND ==
            SHADOW_KIND_ELLIPSOID {
            out.inverse_primary =
                shadow_primitives[instance]
                    .inverse_primary;

            out.inverse_cross =
                shadow_primitives[instance]
                    .inverse_cross;
        } else {
            out.size =
                shadow_primitives[instance]
                    .size_opacity.xyz;

            if SHADOW_PRIMITIVE_KIND !=
                SHADOW_KIND_PARTICLE_SPHERE {
                out.orientation =
                    shadow_primitives[instance]
                        .orientation;
            }
        }
    }

    return out;
}

/// Intersects a local axis slab.
fn shadow_axis_interval(
    origin: f32,
    direction: f32,
    half_extent: f32,
) -> vec2f {
    if abs(direction) <=
        SHADOW_DIRECTION_EPSILON {
        return select(
            infinite_interval(),
            miss_interval(),
            abs(origin) > half_extent,
        );
    }

    let inverse_direction =
        1.0 / direction;

    let roots =
        vec2f(
            -half_extent - origin,
            half_extent - origin,
        ) * inverse_direction;

    return vec2f(
        min(roots.x, roots.y),
        max(roots.x, roots.y),
    );
}

/// Intersects an oriented local box with early interval rejection.
fn shadow_box_interval(
    origin: vec3f,
    direction: vec3f,
    half_size: vec3f,
) -> vec2f {
    var interval =
        shadow_axis_interval(
            origin.x,
            direction.x,
            half_size.x,
        );

    if interval_is_empty(interval) {
        return miss_interval();
    }

    interval =
        interval_intersection(
            interval,
            shadow_axis_interval(
                origin.y,
                direction.y,
                half_size.y,
            ),
        );

    if interval_is_empty(interval) {
        return miss_interval();
    }

    interval =
        interval_intersection(
            interval,
            shadow_axis_interval(
                origin.z,
                direction.z,
                half_size.z,
            ),
        );

    if interval_is_empty(interval)
        || interval.y <= 0.0 {
        return miss_interval();
    }

    return interval;
}

/// Returns the nearest ellipsoid hit.
fn shadow_ellipsoid_t(
    in: ShadowPrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> f32 {
    let offset =
        origin -
        in.world_center;

    let inverse_direction =
        shadow_inverse_tensor_apply(
            in.inverse_primary,
            in.inverse_cross,
            direction,
        );

    let inverse_offset =
        shadow_inverse_tensor_apply(
            in.inverse_primary,
            in.inverse_cross,
            offset,
        );

    let a =
        dot(
            direction,
            inverse_direction,
        );

    if a <= SHADOW_QUADRATIC_EPSILON {
        return -1.0;
    }

    return nearest_positive_interval(
        quadratic_interval(
            a,
            dot(
                offset,
                inverse_direction,
            ),
            dot(
                offset,
                inverse_offset,
            ) - 1.0,
        )
    );
}

/// Transforms the world ray into primitive-local coordinates.
fn shadow_local_ray(
    in: ShadowPrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> mat2x3f {
    let inverse_orientation =
        vec4f(
            -in.orientation.xyz,
            in.orientation.w,
        );

    return mat2x3f(
        shadow_rotate(
            inverse_orientation,
            origin - in.world_center,
        ),
        shadow_rotate(
            inverse_orientation,
            direction,
        ),
    );
}

fn shadow_box_t(
    in: ShadowPrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> f32 {
    let ray =
        shadow_local_ray(
            in,
            origin,
            direction,
        );

    return nearest_positive_interval(
        shadow_box_interval(
            ray[0],
            ray[1],
            in.size * 0.5,
        )
    );
}

fn shadow_particle_sphere_t(
    in: ShadowPrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> f32 {
    let radius =
        in.size.x * 0.5;

    return nearest_positive_interval(
        ray_sphere_interval_precomputed(
            direction,
            in.world_center - origin,
            radius * radius,
            1.0,
        )
    );
}

fn shadow_particle_cylinder_t(
    in: ShadowPrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> f32 {
    let ray =
        shadow_local_ray(
            in,
            origin,
            direction,
        );

    return nearest_positive_interval(
        ray_cylinder_interval(
            ray[0],
            ray[1],
            in.size.x * 0.5,
            in.size.z * 0.5,
        )
    );
}

fn shadow_particle_spherocylinder_t(
    in: ShadowPrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> f32 {
    let ray =
        shadow_local_ray(
            in,
            origin,
            direction,
        );

    let radius =
        in.size.x * 0.5;

    let half_segment =
        max(
            in.size.z * 0.5 -
                radius,
            0.0,
        );

    let endpoint =
        vec3f(
            0.0,
            0.0,
            half_segment,
        );

    return nearest_positive_interval(
        ray_capsule_interval(
            ray[1],
            -endpoint - ray[0],
            endpoint - ray[0],
            radius,
        )
    );
}

/// Compile-time specialized primitive intersection.
fn shadow_primitive_t(
    in: ShadowPrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> f32 {
    switch SHADOW_PRIMITIVE_KIND {
        case SHADOW_KIND_ELLIPSOID: {
            return shadow_ellipsoid_t(
                in,
                origin,
                direction,
            );
        }

        case SHADOW_KIND_BOX: {
            return shadow_box_t(
                in,
                origin,
                direction,
            );
        }

        case SHADOW_KIND_PARTICLE_SPHERE: {
            return shadow_particle_sphere_t(
                in,
                origin,
                direction,
            );
        }

        case SHADOW_KIND_PARTICLE_CYLINDER: {
            return shadow_particle_cylinder_t(
                in,
                origin,
                direction,
            );
        }

        case SHADOW_KIND_PARTICLE_SPHEROCYLINDER: {
            return shadow_particle_spherocylinder_t(
                in,
                origin,
                direction,
            );
        }

        default: {
            return -1.0;
        }
    }
}

@fragment
fn fs_shadow_primitive(
    in: ShadowPrimitiveVsOut,
) -> @builtin(frag_depth) f32 {
    let origin =
        shadow_world_origin(
            in.light_xy
        );

    let t =
        shadow_primitive_t(
            in,
            origin,
            shadow_world_direction(),
        );

    if t <= 0.0 {
        discard;
    }

    // The world ray is the rigid transform of light-space (x, y, -t), so
    // parameter t is preserved and no world->light transform is necessary.
    return shadow_depth(
        vec3f(
            in.light_xy,
            -t,
        )
    );
}
