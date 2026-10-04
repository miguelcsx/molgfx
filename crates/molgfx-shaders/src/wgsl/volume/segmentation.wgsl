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
//!include "include/segmentation/sparse.wgsl"
//!include "include/segmentation/ray.wgsl"
//!include "include/segmentation/label.wgsl"
//!include "include/segmentation/shade.wgsl"
//!include "include/segmentation/integrate.wgsl"
//!include "include/segmentation/surface.wgsl"

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

fn sample_segmentation(
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

    let integration = integrate_segments(ray, interval);
    let accumulated = integration.accumulated;
    let representative_t = integration.representative_t;
    let representative_label = integration.label;
    let representative_coordinate = fma(ray.voxel_direction,
        vec3f(representative_t), ray.voxel_origin);

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
        shade_linear_molecule(
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

@fragment
fn fs_segmentation(in: SegmentationVsOut) -> SegmentationOutput {
    return sample_segmentation(in);
}

struct SegmentationPickOutput {
    @location(0) source_id: u32,
    @location(1) label: u32,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fs_segmentation_pick(in: SegmentationVsOut) -> SegmentationPickOutput {
    let sample = sample_segmentation(in);
    return SegmentationPickOutput(sample.source_id, sample.label, sample.depth);
}

@fragment
fn fs_segmentation_surface_pick(in: BoundaryVsOut,
    @builtin(front_facing) front: bool) -> SegmentationPickOutput {
    let sample = sample_segmentation_surface(in, front);
    return SegmentationPickOutput(sample.source_id, sample.label, sample.depth);
}
