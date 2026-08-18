// Silhouette, cavity and depth cues over the shaded image.
//
// The cues read a four-tap cross of depth and normal, so edge and cavity
// strength come from one small neighbourhood rather than a separate pass.
// The whole block is skipped when a pixel's cues are inactive, keeping the
// cost off fragments that would not show them.

fn illustration_cues_active(
    pixel: vec2i,
    center: vec3f,
    normal: vec3f,
    dimensions: vec2i,
    need_edge: bool,
    need_cavity: bool,
) -> vec2f {
    if !need_edge && !need_cavity {
        return vec2f(0.0);
    }

    let params =
        illustration_depth_params(
            dimensions,
        );

    let center_pixel_zw =
        illustration_pixel_zw(
            pixel,
            params,
        );

    let inv_scale =
        1.0 /
        max(
            abs(center.z),
            MIN_VIEW_DEPTH_SCALE,
        );

    var edge = 0.0;
    var laplacian = 0.0;

    for (
        var index = 0u;
        index < 4u;
        index++
    ) {
        let sample_pixel = clamp(
            pixel +
            ILLUSTRATION_OFFSETS[index],
            vec2i(0),
            dimensions - 1,
        );

        let sample_depth =
            textureLoad(
                depth_texture,
                sample_pixel,
                0,
            );

        if sample_depth <= 0.0 {
            if need_edge {
                edge = 1.0;

                if !need_cavity {
                    break;
                }
            }

            continue;
        }

        let delta_pixel =
            sample_pixel -
            pixel;

        let sample_pixel_zw =
            center_pixel_zw
            + params.pixel_x_zw *
                f32(delta_pixel.x)
            + params.pixel_y_zw *
                f32(delta_pixel.y);

        let sample_z =
            illustration_view_z(
                sample_pixel_zw,
                sample_depth,
                params,
            );

        let depth_delta =
            sample_z -
            center.z;

        if need_cavity {
            laplacian +=
                depth_delta *
                inv_scale;
        }

        // Once silhouette reaches 1, normal/depth-edge work can no longer
        // increase it. Cavity sampling, if requested, still continues.
        if !need_edge || edge >= 1.0 {
            continue;
        }

        let relative_depth =
            abs(depth_delta) *
            inv_scale;

        let depth_edge =
            smoothstep(
                EDGE_DEPTH_LOW,
                EDGE_DEPTH_HIGH,
                relative_depth,
            );

        if depth_edge >= 1.0 {
            edge = 1.0;

            if !need_cavity {
                break;
            }

            continue;
        }

        let sample_normal =
            decode_shading_frame(
                textureLoad(
                    normal_texture,
                    sample_pixel,
                    0,
                ).xyz,
            ).normal;

        let normal_change =
            1.0 -
            clamp(
                dot(
                    normal,
                    sample_normal,
                ),
                -1.0,
                1.0,
            );

        let normal_edge =
            smoothstep(
                EDGE_NORMAL_LOW,
                EDGE_NORMAL_HIGH,
                normal_change,
            );

        edge = max(
            edge,
            max(
                depth_edge,
                normal_edge,
            ),
        );

        if edge >= 1.0
            && !need_cavity {
            break;
        }
    }

    let cavity = select(
        0.0,
        smoothstep(
            CAVITY_LOW,
            CAVITY_HIGH,
            laplacian *
                CROSS_SAMPLE_WEIGHT,
        ),
        need_cavity,
    );

    return vec2f(
        edge,
        cavity,
    );
}

fn illustration_cues(
    pixel: vec2i,
    center: vec3f,
    normal: vec3f,
    dimensions: vec2i,
) -> vec2f {
    return illustration_cues_active(
        pixel,
        center,
        normal,
        dimensions,
        true,
        true,
    );
}

fn apply_illustration(
    color: vec3f,
    pixel: vec2i,
    position: vec3f,
    normal: vec3f,
    dimensions: vec2i,
    uv: vec2f,
) -> vec3f {
    let style =
        frame.illustration.xyz;

    if all(
        style <= vec3f(0.0)
    ) {
        return color;
    }

    var shaped =
        color;

    let need_edge =
        style.x != 0.0;

    let need_cavity =
        style.y != 0.0;

    if need_edge || need_cavity {
        let cues =
            illustration_cues_active(
                pixel,
                position,
                normal,
                dimensions,
                need_edge,
                need_cavity,
            );

        let silhouette =
            min(
                cues.x *
                style.x,
                MAX_SILHOUETTE_DARKENING,
            );

        let cavity =
            min(
                cues.y *
                style.y,
                MAX_CAVITY_DARKENING,
            );

        shaped *=
            (1.0 - silhouette) *
            (1.0 - cavity);
    }

    // Avoid background(), including its length()/smoothstep work, unless
    // depth cueing can actually affect this fragment.
    if style.z == 0.0 {
        return shaped;
    }

    let focus_distance =
        max(
            frame.illustration.w,
            MIN_FOCUS_DISTANCE,
        );

    let depth_cue =
        smoothstep(
            focus_distance *
                DEPTH_CUE_NEAR_FOCUS,

            focus_distance *
                DEPTH_CUE_FAR_FOCUS,

            -position.z,
        ) *
        style.z;

    if depth_cue == 0.0 {
        return shaped;
    }

    return mix(
        shaped,
        background(uv),
        depth_cue,
    );
}
