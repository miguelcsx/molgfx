// Bufferless fullscreen triangle.
//
// Three synthesized vertices cover the viewport. UV is explicitly linear:
// all clip-space W values are 1, so perspective correction is unnecessary.

struct FullscreenOut {
    @builtin(position) position: vec4f,
    @location(0) @interpolate(linear) uv: vec2f,
}

@vertex
fn vs_fullscreen(
    @builtin(vertex_index) vertex: u32,
) -> FullscreenOut {
    let uv = vec2f(
        f32((vertex << 1u) & 2u),
        f32(vertex & 2u),
    );

    return FullscreenOut(
        vec4f(
            uv * 2.0 - vec2f(1.0),
            0.0,
            1.0,
        ),
        vec2f(
            uv.x,
            1.0 - uv.y,
        ),
    );
}
