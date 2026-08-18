// Front-to-back optical integration.
//
// Accumulation is ordered front to back and terminates early once the ray is
// effectively opaque, so a dense map costs less than a transparent one rather
// than always paying the full step count.

fn volume_integrate(
    in: VolumeVsOut,
    ray: VolumeRay,
    interval: vec2f,
) -> OitOutput {
    let medium =
        VOLUME_RENDER_MODE ==
        VOLUME_RENDER_MEDIUM;

    let count =
        transfer_count();

    let step_size =
        max(
            volume.sampling.z *
                volume.sampling.y,
            VOLUME_MIN_STEP,
        );

    let extinction_scale =
        volume.sampling.x *
        step_size /
        max(
            volume.sampling.z,
            VOLUME_MIN_STEP,
        );

    let pixel =
        vec2i(
            in.position.xy
        );

    var hash =
        u32(pixel.x) * 1664525u +
        u32(pixel.y) * 1013904223u +
        u32(frame.temporal.w) *
            747796405u;

    hash =
        (hash ^ (hash >> 16u)) *
        2246822519u;

    hash ^=
        hash >> 13u;

    var t =
        interval.x +
        f32(hash & 0x00FFFFFFu) *
        VOLUME_HASH_SCALE *
        step_size;

    var accumulated =
        vec4f(0.0);

    var representative_t =
        interval.x;

    var representative_voxel =
        fma(
            ray.voxel_direction,
            vec3f(representative_t),
            ray.voxel_origin,
        );

    var cell_exit =
        -VOLUME_DISTANCE_INFINITY;

    var lighting: MediumState;

    if medium {
        lighting =
            medium_state(
                ray.world_direction
            );
    }

    for (
        var step = 0u;
        step < 768u;
        step++
    ) {
        if t > interval.y ||
            accumulated.a >=
                VOLUME_TERMINATION_ALPHA {
            break;
        }

        let coordinate =
            fma(
                ray.voxel_direction,
                vec3f(t),
                ray.voxel_origin,
            );

        if t >= cell_exit {
            let cell =
                empty_space_cell(
                    coordinate,
                    ray,
                );

            cell_exit =
                cell.exit_distance;

            if empty_space_can_skip(
                cell,
                0.0,
                false,
                count,
            ) {
                t =
                    skip_to_cell_exit(
                        t,
                        cell.exit_distance,
                        step_size,
                        interval.y,
                    );

                cell_exit =
                    -VOLUME_DISTANCE_INFINITY;

                continue;
            }
        }

        let transfer =
            transfer_at(
                density_at(coordinate),
                count,
            );

        if transfer.opacity >
            VOLUME_OPACITY_EPSILON {
            let alpha =
                1.0 -
                exp(
                    -transfer.opacity *
                    extinction_scale
                );

            let contribution =
                (1.0 - accumulated.a) *
                alpha;

            if contribution > 0.0 {
                let previous_alpha =
                    accumulated.a;

                var source =
                    transfer.color;

                if medium {
                    source =
                        medium_radiance(
                            source,
                            coordinate,
                            lighting,
                            count,
                        );
                }

                accumulated +=
                    vec4f(
                        contribution * source,
                        contribution,
                    );

                if previous_alpha <
                        VOLUME_REPRESENTATIVE_ALPHA &&
                    accumulated.a >=
                        VOLUME_REPRESENTATIVE_ALPHA {
                    representative_t = t;
                    representative_voxel =
                        coordinate;
                }
            }
        }

        t += step_size;
    }

    if accumulated.a <=
        VOLUME_OPACITY_EPSILON {
        discard;
    }

    let world_position =
        fma(
            ray.world_direction,
            vec3f(representative_t),
            ray.world_origin,
        );

    let view_position =
        volume_transform_point(
            frame.view,
            world_position,
        );

    let base_color =
        accumulated.rgb /
        accumulated.a;

    let depth =
        volume_view_depth(
            view_position
        );

    // Medium does not use a surface normal. Avoid all eight gradient loads.
    if medium {
        return weighted_transparency(
            base_color,
            accumulated.a,
            depth,
        );
    }

    let lit =
        shade_molecule(
            base_color,
            gradient_normal(
                representative_voxel
            ),
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
        accumulated.a,
        depth,
    );
}
