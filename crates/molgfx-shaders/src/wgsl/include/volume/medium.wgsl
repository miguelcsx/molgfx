// Bounded single scattering for participating media.
//
// Scattering is evaluated from explicit caller density and stops at opaque
// molecular depth. It is an environment response for solvent, membrane or
// tomographic context, not a claim that the renderer computed the medium.

// -----------------------------------------------------------------------------
// Medium lighting
// -----------------------------------------------------------------------------

fn medium_state(
    ray_direction: vec3f,
) -> MediumState {
    let light_world =
        volume_transform_direction(
            frame.inv_view,
            MEDIUM_LIGHT_VIEW,
        );

    let light_voxel =
        volume_transform_direction(
            volume.world_to_voxel,
            light_world,
        );

    let light_step =
        light_voxel *
        (
            max(
                volume.sampling.y * 2.0,
                0.5,
            ) *
            inverseSqrt(
                max(
                    dot(light_voxel, light_voxel),
                    VOLUME_RAY_EPSILON,
                )
            )
        );

    let anisotropy = 0.22;

    let cosine =
        dot(
            -ray_direction,
            light_world,
        );

    let denominator =
        max(
            1.0 +
                anisotropy * anisotropy -
                2.0 * anisotropy * cosine,
            1.0e-4,
        );

    let phase =
        (
            1.0 -
            anisotropy * anisotropy
        ) /
        (
            4.0 *
            PI *
            denominator *
            sqrt(denominator)
        );

    return MediumState(
        light_step,
        phase * 12.0,
        select(
            2u,
            6u,
            frame.temporal.y > 0.5,
        ),
    );
}

fn medium_light_transmittance(
    coordinate: vec3f,
    state: MediumState,
    count: u32,
) -> f32 {
    let upper =
        vec3f(
            volume.dimensions.xyz -
            vec3u(1u)
        );

    var point =
        coordinate +
        state.light_step;

    var optical_depth = 0.0;

    for (
        var sample = 0u;
        sample < state.sample_count;
        sample++
    ) {
        if any(point < vec3f(0.0)) ||
            any(point > upper) {
            break;
        }

        let voxel =
            vec3i(
                clamp(
                    round(point),
                    vec3f(0.0),
                    upper,
                )
            );

        optical_depth +=
            transfer_opacity_at(
                textureLoad(
                    density_texture,
                    voxel,
                    0,
                ).x,
                count,
            ) *
            volume.sampling.x;

        point +=
            state.light_step;
    }

    return exp(
        -optical_depth * 0.55
    );
}

fn medium_radiance(
    color: vec3f,
    coordinate: vec3f,
    state: MediumState,
    count: u32,
) -> vec3f {
    return color *
        (
            0.12 +
            state.phase_scale *
            medium_light_transmittance(
                coordinate,
                state,
                count,
            )
        );
}
