// Per-frame camera state and shared camera-space transforms.
//
// Bind group 0 is the per-frame group. Helpers expose common affine
// transformations without computing unused homogeneous components.
//
// Depth is reversed: near = 1, far = 0.

//!include "include/frame_record.wgsl"

@group(0) @binding(0) var<uniform> frame: FrameUniforms;

/// Transforms world position to view space without computing W.
fn camera_view_position(world: vec3f) -> vec3f {
    return frame.view[0].xyz * world.x
        + frame.view[1].xyz * world.y
        + frame.view[2].xyz * world.z
        + frame.view[3].xyz;
}

/// Transforms view position to world space without computing W.
fn camera_world_position(view: vec3f) -> vec3f {
    return frame.inv_view[0].xyz * view.x
        + frame.inv_view[1].xyz * view.y
        + frame.inv_view[2].xyz * view.z
        + frame.inv_view[3].xyz;
}

/// Transforms a world-space direction to view space.
fn camera_view_direction(direction: vec3f) -> vec3f {
    return frame.view[0].xyz * direction.x
        + frame.view[1].xyz * direction.y
        + frame.view[2].xyz * direction.z;
}

/// Transforms a view-space direction to world space.
fn camera_world_direction(direction: vec3f) -> vec3f {
    return frame.inv_view[0].xyz * direction.x
        + frame.inv_view[1].xyz * direction.y
        + frame.inv_view[2].xyz * direction.z;
}

/// Projects a world-space position directly to clip space.
fn camera_world_clip(world: vec3f) -> vec4f {
    return frame.view_proj * vec4f(world, 1.0);
}

/// Projects a view-space position to clip space.
fn camera_view_clip(view: vec3f) -> vec4f {
    return frame.proj * vec4f(view, 1.0);
}

/// Computes only projected Z/W and returns reversed device depth.
fn camera_view_depth(view: vec3f) -> f32 {
    let zw =
        frame.proj[0].zw * view.x
        + frame.proj[1].zw * view.y
        + frame.proj[2].zw * view.z
        + frame.proj[3].zw;

    return zw.x / zw.y;
}

/// Converts NDC XY to top-left-origin pixel coordinates.
fn camera_ndc_to_pixel(ndc: vec2f) -> vec2f {
    return (
        ndc * vec2f(0.5, -0.5)
        + vec2f(0.5)
    ) * frame.viewport.xy;
}

/// Converts top-left-origin pixel coordinates to NDC XY.
fn camera_pixel_to_ndc(pixel: vec2f) -> vec2f {
    return fma(
        pixel,
        frame.viewport.zw * vec2f(2.0, -2.0),
        vec2f(-1.0, 1.0),
    );
}
