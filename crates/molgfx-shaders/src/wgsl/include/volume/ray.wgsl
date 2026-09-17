// Ray normalization, box and clip intervals, and opaque termination.
//
// Rays are unprojected on the four proxy vertices, so a fragment normalizes
// an interpolated ray instead of inverting a projection. The interval is
// narrowed by the crop box, the clip planes and opaque molecular depth before
// the first sample, so cropping cuts fragment work and not just visibility.

/// Normalizes the interpolated ray once while preserving one common t-space.
fn volume_ray(in: VolumeVsOut) -> VolumeRay {
    let inverse_length =
        inverseSqrt(
            max(
                dot(
                    in.world_vector,
                    in.world_vector,
                ),
                VOLUME_RAY_EPSILON,
            )
        );

    let world_direction =
        in.world_vector *
        inverse_length;

    let voxel_direction =
        in.voxel_vector *
        inverse_length;

    return VolumeRay(
        in.world_origin,
        world_direction,
        in.voxel_origin,
        voxel_direction,
        1.0 / voxel_direction,
        in.view_z.x,
        in.view_z.y * inverse_length,
    );
}

fn volume_ray_box(
    origin: vec3f,
    inverse_direction: vec3f,
    lower: vec3f,
    upper: vec3f,
) -> vec2f {
    let a =
        (lower - origin) *
        inverse_direction;

    let b =
        (upper - origin) *
        inverse_direction;

    let near =
        min(a, b);

    let far =
        max(a, b);

    return vec2f(
        max(max(near.x, near.y), near.z),
        min(min(far.x, far.y), far.z),
    );
}

/// Tightens the world-ray interval against optional clipping planes.
fn volume_clip_interval(
    ray: VolumeRay,
    input: vec2f,
) -> vec2f {
    if !VOLUME_CLIPPING_ENABLED {
        return input;
    }

    var interval =
        input;

    let count =
        min(volume.clip_meta.x, 4u);

    for (var index = 0u; index < count; index++) {
        let plane =
            volume.clip_planes[index];

        let origin_distance =
            dot(
                plane.xyz,
                ray.world_origin,
            ) + plane.w;

        let denominator =
            dot(
                plane.xyz,
                ray.world_direction,
            );

        if denominator > VOLUME_RAY_EPSILON {
            interval.x =
                max(
                    interval.x,
                    -origin_distance /
                        denominator,
                );
        } else if denominator < -VOLUME_RAY_EPSILON {
            interval.y =
                min(
                    interval.y,
                    -origin_distance /
                        denominator,
                );
        } else if origin_distance < 0.0 {
            return vec2f(1.0, 0.0);
        }

        if interval.x >= interval.y {
            return vec2f(1.0, 0.0);
        }
    }

    return interval;
}

/// Reconstructs only view-space Z from the opaque depth buffer.
fn volume_view_z(
    pixel: vec2i,
    depth: f32,
) -> f32 {
    let ndc =
        fma(
            vec2f(pixel) + vec2f(0.5),
            frame.viewport.zw *
                vec2f(2.0, -2.0),
            vec2f(-1.0, 1.0),
        );

    let zw =
        frame.inv_proj[0].zw * ndc.x
        + frame.inv_proj[1].zw * ndc.y
        + frame.inv_proj[2].zw * depth
        + frame.inv_proj[3].zw;

    return zw.x *
        (
            sign(zw.y) /
            max(
                abs(zw.y),
                VOLUME_RAY_EPSILON,
            )
        );
}

/// Converts opaque depth to distance along the already-known view ray.
fn opaque_distance(
    pixel: vec2i,
    ray: VolumeRay,
) -> f32 {
    let depth =
        textureLoad(
            oit_depth_texture,
            pixel,
            0,
        );

    if depth <= 0.0 ||
        abs(ray.view_direction_z) <=
            VOLUME_RAY_EPSILON {
        return VOLUME_DISTANCE_INFINITY;
    }

    return (
        volume_view_z(
            pixel,
            depth,
        ) -
        ray.view_origin_z
    ) / ray.view_direction_z;
}
