// Ray setup, box and clip intervals, and opaque-depth termination.
//
// The ray interval is narrowed by the crop box, the clip planes and the
// opaque molecular depth before a single label is fetched, so cropping and
// clipping reduce fragment work rather than only hiding output.

fn segmentation_ray(
    in: SegmentationVsOut,
) -> SegmentationRay {
    let inverse_length =
        inverseSqrt(
            max(
                dot(
                    in.world_vector,
                    in.world_vector,
                ),
                SEGMENT_RAY_EPSILON,
            )
        );

    let world_direction =
        in.world_vector *
        inverse_length;

    let voxel_direction =
        in.voxel_vector *
        inverse_length;

    return SegmentationRay(
        in.world_origin,
        world_direction,
        in.voxel_origin,
        voxel_direction,
        1.0 / voxel_direction,
        in.view_z.x,
        in.view_z.y * inverse_length,
    );
}

fn segmentation_ray_box(
    ray: SegmentationRay,
    lower: vec3f,
    upper: vec3f,
) -> vec2f {
    let a =
        (lower - ray.voxel_origin) *
        ray.voxel_inverse_direction;

    let b =
        (upper - ray.voxel_origin) *
        ray.voxel_inverse_direction;

    let near =
        min(a, b);

    let far =
        max(a, b);

    return vec2f(
        max(max(near.x, near.y), near.z),
        min(min(far.x, far.y), far.z),
    );
}

fn segmentation_clip_interval(
    ray: SegmentationRay,
    input: vec2f,
) -> vec2f {
    if !SEGMENTATION_CLIPPING_ENABLED {
        return input;
    }

    var interval =
        input;

    let count =
        min(volume.clip_meta.x, 4u);

    for (var index = 0u; index < count; index++) {
        let plane =
            volume.clip_planes[index];

        let distance =
            dot(
                plane.xyz,
                ray.world_origin,
            ) + plane.w;

        let denominator =
            dot(
                plane.xyz,
                ray.world_direction,
            );

        if denominator > SEGMENT_RAY_EPSILON {
            interval.x =
                max(
                    interval.x,
                    -distance / denominator,
                );
        } else if denominator < -SEGMENT_RAY_EPSILON {
            interval.y =
                min(
                    interval.y,
                    -distance / denominator,
                );
        } else if distance < 0.0 {
            return vec2f(1.0, 0.0);
        }

        if interval.x >= interval.y {
            return vec2f(1.0, 0.0);
        }
    }

    return interval;
}

fn segmentation_view_z(
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
            max(abs(zw.y), SEGMENT_RAY_EPSILON)
        );
}

fn segmentation_opaque_distance(
    pixel: vec2i,
    ray: SegmentationRay,
) -> f32 {
    let depth =
        textureLoad(
            oit_depth_texture,
            pixel,
            0,
        );

    if depth <= 0.0 ||
        abs(ray.view_direction_z) <=
            SEGMENT_RAY_EPSILON {
        return SEGMENT_INFINITY;
    }

    return (
        segmentation_view_z(
            pixel,
            depth,
        ) -
        ray.view_origin_z
    ) / ray.view_direction_z;
}
