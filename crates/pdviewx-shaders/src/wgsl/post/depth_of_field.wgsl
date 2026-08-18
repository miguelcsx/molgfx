// Tile-classified, occlusion-aware thin-lens depth of field.
//
// Classification exits immediately once the tile reaches the maximum CoC.
// Resolve avoids tile-dimension queries and per-tap clamps for interior pixels.

//!include "include/camera.wgsl"
//!include "include/fullscreen.wgsl"
//!include "include/dof/optics.wgsl"
//!include "include/dof/gather.wgsl"

@fragment
fn fs_classify_dof(
    in: FullscreenOut,
) -> @location(0) f32 {
    let max_radius =
        frame.optics.z;

    if max_radius <= 0.0 {
        return 0.0;
    }

    let dimensions =
        vec2i(
            textureDimensions(
                classify_depth
            )
        );

    let origin =
        vec2i(in.position.xy) *
        TILE_SIZE;

    let extent =
        clamp(
            dimensions - origin,
            vec2i(0),
            vec2i(TILE_SIZE),
        );

    if any(extent <= vec2i(0)) {
        return 0.0;
    }

    let depth_params =
        dof_view_depth_params(
            dimensions
        );

    let coc_params =
        dof_coc_params();

    let result_scale =
        1.0 /
        max(
            max_radius,
            1.0,
        );

    var row_zw =
        depth_params.bias_zw +
        depth_params.pixel_x_zw *
            f32(origin.x) +
        depth_params.pixel_y_zw *
            f32(origin.y);

    var maximum = 0.0;

    for (
        var y = 0i;
        y < extent.y;
        y++
    ) {
        var pixel_zw =
            row_zw;

        for (
            var x = 0i;
            x < extent.x;
            x++
        ) {
            let raw_depth =
                textureLoad(
                    classify_depth,
                    origin + vec2i(x, y),
                    0,
                );

            if raw_depth > 0.0 {
                let depth =
                    dof_depth_from_zw(
                        pixel_zw +
                        depth_params.depth_zw *
                            raw_depth,
                    );

                maximum =
                    max(
                        maximum,
                        abs(
                            dof_signed_coc_radius(
                                depth,
                                coc_params,
                            )
                        ),
                    );

                // CoC is capped to max_radius: no remaining sample can win.
                if maximum >= max_radius {
                    return maximum *
                        result_scale;
                }
            }

            pixel_zw +=
                depth_params.pixel_x_zw;
        }

        row_zw +=
            depth_params.pixel_y_zw;
    }

    return maximum *
        result_scale;
}

@fragment
fn fs_resolve_dof(
    in: FullscreenOut,
) -> @location(0) vec4f {
    // Contract: resolved buffers match the current viewport.
    let dimensions =
        vec2i(frame.viewport.xy);

    let pixel =
        vec2i(in.position.xy);

    let center =
        textureLoad(
            resolved_hdr,
            pixel,
            0,
        ).rgb;

    let max_radius =
        frame.optics.z;

    if max_radius <= 0.0 {
        return vec4f(
            center,
            1.0,
        );
    }

    // tile_coc is ceil(viewport / TILE_SIZE), so this is already in bounds.
    let tile =
        pixel / TILE_SIZE;

    let tile_radius =
        textureLoad(
            tile_coc,
            tile,
            0,
        ).r *
        max_radius;

    if tile_radius <
        SHARP_RADIUS_PIXELS {
        return vec4f(
            center,
            1.0,
        );
    }

    let center_raw_depth =
        textureLoad(
            resolved_depth,
            pixel,
            0,
        );

    if center_raw_depth <= 0.0 {
        return vec4f(
            center,
            1.0,
        );
    }

    let depth_params =
        dof_view_depth_params(
            dimensions
        );

    let coc_params =
        dof_coc_params();

    let center_depth =
        dof_view_depth(
            pixel,
            center_raw_depth,
            depth_params,
        );

    let center_coc =
        dof_signed_coc_radius(
            center_depth,
            coc_params,
        );

    let polygon =
        dof_polygon_params(
            frame.optics.w
        );

    // Aperture samples are bounded to the tile radius.
    let margin =
        i32(
            ceil(tile_radius)
        );

    let interior =
        all(pixel >= vec2i(margin)) &&
        all(
            pixel <
            dimensions - vec2i(margin)
        );

    var accumulation =
        center;

    var total_weight =
        1.0;

    for (
        var index = 0u;
        index < GATHER_TAPS;
        index++
    ) {
        let offset =
            vec2i(
                round(
                    dof_aperture_sample(
                        index,
                        polygon,
                    ) *
                    tile_radius
                )
            );

        var sample_pixel =
            pixel + offset;

        if !interior {
            sample_pixel =
                clamp(
                    sample_pixel,
                    vec2i(0),
                    dimensions - 1,
                );
        }

        let sample_raw_depth =
            textureLoad(
                resolved_depth,
                sample_pixel,
                0,
            );

        if sample_raw_depth <= 0.0 {
            continue;
        }

        let sample_depth =
            dof_view_depth(
                sample_pixel,
                sample_raw_depth,
                depth_params,
            );

        let sample_coc =
            dof_signed_coc_radius(
                sample_depth,
                coc_params,
            );

        let offset_float =
            vec2f(offset);

        let distance_sq =
            dot(
                offset_float,
                offset_float,
            );

        let coverage_radius =
            dof_sample_coverage_radius(
                sample_coc,
                center_coc,
                distance_sq,
            );

        if coverage_radius <= 0.0 {
            continue;
        }

        let weight =
            dof_coverage_weight(
                coverage_radius,
                sqrt(distance_sq),
            );

        if weight <= 0.0 {
            continue;
        }

        // HDR is intentionally the final fetch after all cheaper rejections.
        accumulation +=
            textureLoad(
                resolved_hdr,
                sample_pixel,
                0,
            ).rgb *
            weight;

        total_weight +=
            weight;
    }

    return vec4f(
        accumulation /
            total_weight,
        1.0,
    );
}
