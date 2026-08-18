// Categorical normals, slice sampling and accumulated output.
//
// Accumulation is commutative weighted-blended coverage, so overlapping
// regions need no sort. A slice samples the same grid on a plane rather than
// integrating along it, keeping the two readings of one map distinct.

}

fn segment_normal(
    coordinate: vec3f,
    label: u32,
) -> vec3f {
    let px =
        select(
            0.0,
            1.0,
            label_at_clamped(
                coordinate +
                vec3f(1.0, 0.0, 0.0)
            ) != label,
        );

    let nx =
        select(
            0.0,
            1.0,
            label_at_clamped(
                coordinate -
                vec3f(1.0, 0.0, 0.0)
            ) != label,
        );

    let py =
        select(
            0.0,
            1.0,
            label_at_clamped(
                coordinate +
                vec3f(0.0, 1.0, 0.0)
            ) != label,
        );

    let ny =
        select(
            0.0,
            1.0,
            label_at_clamped(
                coordinate -
                vec3f(0.0, 1.0, 0.0)
            ) != label,
        );

    let pz =
        select(
            0.0,
            1.0,
            label_at_clamped(
                coordinate +
                vec3f(0.0, 0.0, 1.0)
            ) != label,
        );

    let nz =
        select(
            0.0,
            1.0,
            label_at_clamped(
                coordinate -
                vec3f(0.0, 0.0, 1.0)
            ) != label,
        );

    let local_gradient =
        -vec3f(
            px - nx,
            py - ny,
            pz - nz,
        );

    let world_normal =
        (
            transpose(
                volume.world_to_voxel
            ) *
            vec4f(
                local_gradient,
                0.0,
            )
        ).xyz;

    let view_normal =
        segment_transform_direction(
            frame.view,
            world_normal,
        );

    let length_sq =
        dot(
            view_normal,
            view_normal,
        );

    if length_sq <= 1.0e-10 {
        return vec3f(0.0, 0.0, 1.0);
    }

    return view_normal *
        inverseSqrt(length_sq);
}

fn segmentation_depth(
    view_position: vec3f,
) -> f32 {
    let zw =
        frame.proj[0].zw * view_position.x
        + frame.proj[1].zw * view_position.y
        + frame.proj[2].zw * view_position.z
        + frame.proj[3].zw;

    return zw.x / zw.y;
}

fn output_at(
    color: vec3f,
    opacity: f32,
    depth: f32,
    label: u32,
) -> SegmentationOutput {
    let oit =
        weighted_transparency(
            color,
            opacity,
            depth,
        );

    return SegmentationOutput(
        oit.accumulation,
        oit.revealage,
        volume.lookup.w,
        label,
        oit.depth,
    );
}

fn segmentation_slice(
    in: SegmentationVsOut,
    ray: SegmentationRay,
    interval: vec2f,
) -> SegmentationOutput {
    let plane =
        volume.slice_plane;

    let denominator =
        dot(
            plane.xyz,
            ray.world_direction,
        );

    if abs(denominator) <=
        SEGMENT_RAY_EPSILON {
        discard;
    }

    let t =
        -(
            dot(
                plane.xyz,
                ray.world_origin,
            ) + plane.w
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

    let label =
        label_at(coordinate);

    let style =
        sample_style(label);

    if !style.found {
        discard;
    }

    let opacity =
        clamp(
            style.opacity *
                volume.sampling.x,
            0.0,
            1.0,
        );

    if opacity <=
        SEGMENT_OPACITY_EPSILON {
        discard;
    }

    let world_position =
        fma(
            ray.world_direction,
            vec3f(t),
            ray.world_origin,
        );

    let view_position =
        segment_transform_point(
            frame.view,
            world_position,
        );

    let color =
        shade_molecule(
            style.color,
            segment_normal(
                coordinate,
                label,
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

    return output_at(
        color,
        opacity,
        segmentation_depth(
            view_position
        ),
        label,
    );
}
