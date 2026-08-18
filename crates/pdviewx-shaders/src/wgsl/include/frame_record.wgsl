// Per-frame matrices and viewport state shared by graphics and compute.
//
// Layout is intentionally 16-byte aligned and byte-stable with the host.
// Total uniform size: 1024 bytes.

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
    illustration: vec4f,
    optics: vec4f,
    motion_blur: vec4f,
    projection_kind: vec4f,

    shadow_view: mat4x4f,
    shadow_inv_view: mat4x4f,
    shadow_projection: mat4x4f,
    shadow_view_proj: mat4x4f,

    atmosphere: array<vec4f, 6>,
    lighting: array<vec4f, 8>,
}
