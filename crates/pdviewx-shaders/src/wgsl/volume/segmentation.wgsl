// Caller-supplied categorical label volumes.
//
// Rays are unprojected on four proxy vertices. Lookup strategy and slice mode
// are pipeline constants. Consecutive equal labels reuse their resolved style.
//
// Labels are exact caller data, so every stage under include/segmentation/
// reads them with nearest integer loads: interpolating a label would invent a
// category the caller never supplied.

//!include "include/camera.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/oit_input.wgsl"
//!include "include/segmentation/types.wgsl"
//!include "include/segmentation/ray.wgsl"
//!include "include/segmentation/label.wgsl"
//!include "include/segmentation/shade.wgsl"

@vertex
fn vs_segmentation(
    @builtin(vertex_index) vertex: u32,
) -> SegmentationVsOut {
    let bounds =
        segmentation_bounds();

    let ndc =
        mix(
            bounds[0],
            bounds[1],
            segment_quad_uv(vertex),
        );

    let near_view =
        segment_homogeneous(
            frame.inv_proj *
            vec4f(ndc, 1.0, 1.0)
        );

    let far_view =
        segment_homogeneous(
            frame.inv_proj *
            vec4f(ndc, 0.0, 1.0)
        );

    let near_world =
        segment_transform_point(
            frame.inv_view,
            near_view,
        );

    let far_world =
        segment_transform_point(
            frame.inv_view,
            far_view,
        );

    let near_voxel =
        segment_transform_point(
            volume.world_to_voxel,
            near_world,
        );

    let far_voxel =
        segment_transform_point(
            volume.world_to_voxel,
            far_world,
        );

    return SegmentationVsOut(
        vec4f(ndc, 0.0, 1.0),
        near_world,
        far_world - near_world,
        near_voxel,
        far_voxel - near_voxel,
        vec2f(
            near_view.z,
            far_view.z - near_view.z,
        ),
    );
}

@fragment
fn fs_segmentation(
    in: SegmentationVsOut,
) -> SegmentationOutput {
    let ray =
        segmentation_ray(in);

    var interval =
        segmentation_ray_box(
            ray,
            vec3f(
                volume.crop_minimum.xyz
            ),
            vec3f(
                volume.crop_maximum.xyz -
                vec3u(1u)
            ),
        );

    interval =
        segmentation_clip_interval(
            ray,
            interval,
        );

    interval.x =
        max(
            interval.x,
            0.0,
        );

    interval.y =
        min(
            interval.y,
            segmentation_opaque_distance(
                vec2i(in.position.xy),
                ray,
            ),
        );

    if interval.x >= interval.y {
        discard;
    }

    if SEGMENTATION_SLICE_MODE {
        return segmentation_slice(
            in,
            ray,
            interval,
        );
    }

    let step_size =
        max(
            volume.sampling.z *
                volume.sampling.y,
            SEGMENT_MIN_STEP,
        );

    let extinction_scale =
        volume.sampling.x *
        step_size /
        max(
            volume.sampling.z,
            SEGMENT_MIN_STEP,
        );

    let pixel =
        vec2u(
            vec2i(in.position.xy)
        );

    var hash =
        pixel.x * 1664525u +
        pixel.y * 1013904223u +
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
        SEGMENT_HASH_SCALE *
        step_size;

    var accumulated =
        vec4f(0.0);

    var representative_t =
        interval.x;

    var representative_coordinate =
        fma(
            ray.voxel_direction,
            vec3f(representative_t),
            ray.voxel_origin,
        );

    var representative_label = 0u;

    // Cache label -> style across consecutive samples.
    var cached_label = 0u;
    var cached_style =
        absent_style();

    var cache_valid = false;

    for (
        var step = 0u;
        step < 768u;
        step++
    ) {
        if t > interval.y ||
            accumulated.a >=
                SEGMENT_TERMINATION_ALPHA {
            break;
        }

        let coordinate =
            fma(
                ray.voxel_direction,
                vec3f(t),
                ray.voxel_origin,
            );

        let label =
            label_at(coordinate);

        if !cache_valid ||
            label != cached_label {
            cached_label = label;
            cached_style =
                sample_style(label);
            cache_valid = true;
        }

        if cached_style.found &&
            cached_style.opacity >
                SEGMENT_OPACITY_EPSILON {
            let alpha =
                1.0 -
                exp(
                    -cached_style.opacity *
                    extinction_scale
                );

            let contribution =
                (1.0 - accumulated.a) *
                alpha;

            if contribution > 0.0 {
                let previous_alpha =
                    accumulated.a;

                accumulated +=
                    vec4f(
                        contribution *
                            cached_style.color,
                        contribution,
                    );

                if previous_alpha <
                        SEGMENT_REPRESENTATIVE_ALPHA &&
                    accumulated.a >=
                        SEGMENT_REPRESENTATIVE_ALPHA {
                    representative_t = t;
                    representative_coordinate =
                        coordinate;
                    representative_label =
                        label;
                }
            }
        }

        t += step_size;
    }

    if accumulated.a <=
        SEGMENT_OPACITY_EPSILON {
        discard;
    }

    let world_position =
        fma(
            ray.world_direction,
            vec3f(representative_t),
            ray.world_origin,
        );

    let view_position =
        segment_transform_point(
            frame.view,
            world_position,
        );

    let lit =
        shade_molecule(
            accumulated.rgb /
                accumulated.a,
            segment_normal(
                representative_coordinate,
                representative_label,
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
        lit,
        accumulated.a,
        segmentation_depth(
            view_position
        ),
        representative_label,
    );
}
