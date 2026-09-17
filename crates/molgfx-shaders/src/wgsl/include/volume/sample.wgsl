// Isosurface refinement, slice sampling and true hit depth.
//
// An isosurface hit is bracketed at the first scalar crossing and binary
// refined, so its depth is the real boundary rather than a step position. A
// slice reads the grid on a plane instead of integrating along it, keeping
// the two readings of one map distinct.

// -----------------------------------------------------------------------------
// Hit refinement / depth
// -----------------------------------------------------------------------------

fn refine_isosurface(
    ray: VolumeRay,
    low_start: f32,
    high_start: f32,
    low_inside: bool,
    level: f32,
) -> f32 {
    var low =
        low_start;

    var high =
        high_start;

    for (
        var iteration = 0u;
        iteration < 8u;
        iteration++
    ) {
        let middle =
            (low + high) * 0.5;

        let inside =
            density_at(
                fma(
                    ray.voxel_direction,
                    vec3f(middle),
                    ray.voxel_origin,
                )
            ) >= level;

        if inside == low_inside {
            low = middle;
        } else {
            high = middle;
        }
    }

    return (low + high) * 0.5;
}

fn volume_view_depth(
    view_position: vec3f,
) -> f32 {
    let zw =
        frame.proj[0].zw * view_position.x
        + frame.proj[1].zw * view_position.y
        + frame.proj[2].zw * view_position.z
        + frame.proj[3].zw;

    return zw.x / zw.y;
}

// -----------------------------------------------------------------------------
// Slice
// -----------------------------------------------------------------------------

fn slice_sample(
    in: VolumeVsOut,
    ray: VolumeRay,
    interval: vec2f,
) -> OitOutput {
    let plane =
        volume.slice_plane;

    let denominator =
        dot(
            plane.xyz,
            ray.world_direction,
        );

    if abs(denominator) <=
        VOLUME_RAY_EPSILON {
        discard;
    }

    let t =
        -(
            dot(
                plane.xyz,
                ray.world_origin,
            ) +
            plane.w
        ) / denominator;

    if t < interval.x ||
        t > interval.y {
        discard;
    }

    let coordinate =
        fma(
            ray.voxel_direction,
            vec3f(t),
            ray.voxel_origin,
        );

    let transfer =
        transfer_at(
            density_at(coordinate),
            transfer_count(),
        );

    let opacity =
        clamp(
            transfer.opacity *
                volume.sampling.x,
            0.0,
            1.0,
        );

    if opacity <=
        VOLUME_OPACITY_EPSILON {
        discard;
    }

    let world_position =
        fma(
            ray.world_direction,
            vec3f(t),
            ray.world_origin,
        );

    let view_position =
        volume_transform_point(
            frame.view,
            world_position,
        );

    return weighted_transparency(
        transfer.color,
        opacity,
        volume_view_depth(
            view_position
        ),
    );
}

// -----------------------------------------------------------------------------
// Isosurface / liquid
// -----------------------------------------------------------------------------

fn isosurface_sample(
    in: VolumeVsOut,
    ray: VolumeRay,
    interval: vec2f,
) -> OitOutput {
    let liquid =
        VOLUME_RENDER_MODE ==
        VOLUME_RENDER_LIQUID;

    let level =
        volume.scalar.z;

    let count =
        transfer_count();

    let step_size =
        max(
            volume.sampling.z *
                volume.sampling.y,
            VOLUME_MIN_STEP,
        );

    var previous_t =
        interval.x;

    var previous_value =
        density_at(
            fma(
                ray.voxel_direction,
                vec3f(previous_t),
                ray.voxel_origin,
            )
        ) - level;

    var t =
        min(
            previous_t + step_size,
            interval.y,
        );

    var hit_t =
        previous_t;

    var found =
        previous_value >= 0.0;

    var cell_exit =
        -VOLUME_DISTANCE_INFINITY;

    for (
        var step = 0u;
        step < 768u &&
            !found;
        step++
    ) {
        if previous_t >= cell_exit {
            let cell =
                empty_space_cell(
                    fma(
                        ray.voxel_direction,
                        vec3f(previous_t),
                        ray.voxel_origin,
                    ),
                    ray,
                );

            cell_exit =
                cell.exit_distance;

            if empty_space_can_skip(
                cell,
                level,
                true,
                count,
            ) {
                previous_t =
                    skip_to_cell_exit(
                        previous_t,
                        cell.exit_distance,
                        step_size,
                        interval.y,
                    );

                if previous_t >=
                    interval.y {
                    break;
                }

                previous_value =
                    density_at(
                        fma(
                            ray.voxel_direction,
                            vec3f(previous_t),
                            ray.voxel_origin,
                        )
                    ) - level;

                t =
                    min(
                        previous_t +
                            step_size,
                        interval.y,
                    );

                cell_exit =
                    -VOLUME_DISTANCE_INFINITY;

                continue;
            }
        }

        let value =
            density_at(
                fma(
                    ray.voxel_direction,
                    vec3f(t),
                    ray.voxel_origin,
                )
            ) - level;

        if value == 0.0 {
            hit_t = t;
            found = true;
            break;
        }

        if (value >= 0.0) !=
            (previous_value >= 0.0) {
            hit_t =
                refine_isosurface(
                    ray,
                    previous_t,
                    t,
                    previous_value >= 0.0,
                    level,
                );

            found = true;
            break;
        }

        if t >= interval.y {
            break;
        }

        previous_t = t;
        previous_value = value;

        t =
            min(
                t + step_size,
                interval.y,
            );
    }

    if !found {
        discard;
    }

    let coordinate =
        fma(
            ray.voxel_direction,
            vec3f(hit_t),
            ray.voxel_origin,
        );

    let world_position =
        fma(
            ray.world_direction,
            vec3f(hit_t),
            ray.world_origin,
        );

    let view_position =
        volume_transform_point(
            frame.view,
            world_position,
        );

    let normal =
        gradient_normal(coordinate);

    let transfer =
        transfer_at(
            level,
            count,
        );

    var color =
        transfer.color;

    if liquid {
        let view_direction =
            normalize(
                -view_position
            );

        let rim_base =
            1.0 -
            max(
                dot(
                    normal,
                    view_direction,
                ),
                0.0,
            );

        let rim =
            rim_base *
            rim_base *
            rim_base;

        color =
            mix(
                color,
                vec3f(0.86, 0.96, 1.0),
                rim * 0.24,
            );
    }

    let lit =
        shade_molecule(
            color,
            normal,
            volume.material.x,
            material_payload(
                volume.material
            ),
            view_position,
            oit_occlusion(
                in.position
            ),
        );

    return weighted_transparency(
        lit,
        volume.scalar.w,
        volume_view_depth(
            view_position
        ),
    );
}
