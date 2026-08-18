// Weighted-blended order-independent transparency accumulation.
//
// Coverage is weighted by depth and accumulated commutatively, so overlapping
// transparent molecular surfaces need no per-frame sort and no per-pixel
// list — the composite resolves in one pass regardless of draw order.

struct OitOutput {
    @location(0) accumulation: vec4f,
    @location(1) revealage: f32,
    @builtin(frag_depth) depth: f32,
}

fn weighted_transparency(
    color: vec3f,
    opacity: f32,
    depth: f32,
) -> OitOutput {
    let alpha =
        clamp(
            opacity,
            0.0,
            1.0,
        );

    let alpha_squared =
        alpha * alpha;

    let depth_term =
        0.15 +
        depth;

    let depth_squared =
        depth_term *
        depth_term;

    let weight =
        clamp(
            alpha *
                alpha_squared *
                3000.0 *
                depth_term *
                depth_squared,
            0.01,
            3000.0,
        );

    return OitOutput(
        vec4f(
            color * alpha,
            alpha,
        ) * weight,
        alpha,
        depth,
    );
}
