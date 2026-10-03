// Per-frame matrices and viewport state shared by graphics and compute.
//
// Layout is intentionally 16-byte aligned and byte-stable with the host.
// Total uniform size: 1056 bytes.

struct FrameUniforms {
    view: mat4x4f,
    inv_view: mat4x4f,
    proj: mat4x4f,
    view_proj: mat4x4f,
    inv_proj: mat4x4f,
    reprojection: mat4x4f,
    previous_view_proj: mat4x4f,

    viewport: vec4f,
    temporal: vec4f,
    shape_cues: vec4f,
    depth_cue: vec4f,
    npr: vec4f,
    optics: vec4f,
    motion_blur: vec4f,
    // x = 0 perspective / 1 orthographic; yz = sphere/frustum factors.
    projection_kind: vec4f,

    shadow_view: mat4x4f,
    shadow_inv_view: mat4x4f,
    shadow_projection: mat4x4f,
    shadow_view_proj: mat4x4f,

    atmosphere: array<vec4f, 6>,
    lighting: array<vec4f, 8>,
}

/// View-space near plane from the reversed-Z projection equation z = w.
fn projection_near_z(projection: mat4x4f) -> f32 {
    return (projection[3].w - projection[3].z) /
        (projection[2].z - projection[2].w);
}
