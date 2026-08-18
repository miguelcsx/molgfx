// Caller-supplied scalar-grid rendering.
//
// Rendering mode is specialized per pipeline. Rays are unprojected on the
// four proxy vertices; fragments only normalize the interpolated ray.
//
// The trilinear field gradient reuses one eight-texel cell instead of six
// independent trilinear samples (48 texture loads).
//
// This file holds the two pipeline stages; the field sampling, transfer,
// traversal and integration they compose live beside it under include/volume/.
// The caller's grid stays authoritative throughout: nothing here invents
// density the caller did not supply.

//!include "include/camera.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/oit_input.wgsl"
//!include "include/volume/types.wgsl"
//!include "include/volume/ray.wgsl"
//!include "include/volume/density.wgsl"
//!include "include/volume/transfer.wgsl"
//!include "include/volume/skip.wgsl"
//!include "include/volume/medium.wgsl"
//!include "include/volume/sample.wgsl"
//!include "include/volume/integrate.wgsl"

/// Unprojects one proxy vertex. Per-pixel inverse projection is unnecessary.
@vertex
fn vs_volume(
    @builtin(vertex_index) vertex: u32,
) -> VolumeVsOut {
    let bounds =
        volume_ndc_bounds();

    let ndc =
        mix(
            bounds[0],
            bounds[1],
            volume_quad_uv(vertex),
        );

    let near_view =
        volume_homogeneous_point(
            frame.inv_proj *
            vec4f(ndc, 1.0, 1.0)
        );

    let far_view =
        volume_homogeneous_point(
            frame.inv_proj *
            vec4f(ndc, 0.0, 1.0)
        );

    let near_world =
        volume_transform_point(
            frame.inv_view,
            near_view,
        );

    let far_world =
        volume_transform_point(
            frame.inv_view,
            far_view,
        );

    let near_voxel =
        volume_transform_point(
            volume.world_to_voxel,
            near_world,
        );

    let far_voxel =
        volume_transform_point(
            volume.world_to_voxel,
            far_world,
        );

    return VolumeVsOut(
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
fn fs_volume(
    in: VolumeVsOut,
) -> OitOutput {
    let ray =
        volume_ray(in);

    var interval =
        volume_ray_box(
            ray.voxel_origin,
            ray.voxel_inverse_direction,
            vec3f(
                volume.crop_minimum.xyz
            ),
            vec3f(
                volume.crop_maximum.xyz -
                vec3u(1u)
            ),
        );

    interval =
        volume_clip_interval(
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
            opaque_distance(
                vec2i(in.position.xy),
                ray,
            ),
        );

    if interval.x >= interval.y {
        discard;
    }

    switch VOLUME_RENDER_MODE {
        case VOLUME_RENDER_SLICE: {
            return slice_sample(
                in,
                ray,
                interval,
            );
        }

        case VOLUME_RENDER_ISOSURFACE,
             VOLUME_RENDER_LIQUID: {
            return isosurface_sample(
                in,
                ray,
                interval,
            );
        }

        default: {
            return volume_integrate(
                in,
                ray,
                interval,
            );
        }
    }
}
