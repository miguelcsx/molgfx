// Subject coverage and final presentation adjustments.
//
// Contrast and saturation are applied around a fixed pivot so the adjustment
// is reversible and does not shift neutral tones, keeping the presented image
// tied to the rendered one rather than stylizing it.

fn subject_coverage(
    pixel: vec2i,
) -> f32 {
    let depth =
        textureLoad(
            depth_texture,
            pixel,
            0,
        ).x;

    let opaque =
        select(
            0.0,
            1.0,
            depth > 0.0,
        );

    if frame.atmosphere[4].z <= 0.5 {
        return opaque;
    }

    let revealage = clamp(
        textureLoad(
            revealage_texture,
            pixel,
            0,
        ).r,
        0.0,
        1.0,
    );

    return max(
        opaque,
        1.0 - revealage,
    );
}

fn presentation_coverage(
    pixel: vec2i,
    uv_y: f32,
) -> f32 {
    let backdrop = mix(
        frame.atmosphere[4].x,
        frame.atmosphere[4].y,
        smoothstep(
            0.0,
            1.0,
            uv_y,
        ),
    );

    // A fully opaque backdrop makes subject coverage irrelevant.
    if backdrop >= 1.0 {
        return 1.0;
    }

    let subject =
        subject_coverage(pixel);

    return subject +
        (1.0 - subject) *
        backdrop;
}

fn apply_display_adjustments(
    color: vec3f,
    uv: vec2f,
) -> vec3f {
    let luminance =
        dot(
            color,
            LUMINANCE_WEIGHTS,
        );

    var display = mix(
        vec3f(luminance),
        color,
        frame.atmosphere[3].x,
    );

    display = clamp(
        (
            display -
            CONTRAST_PIVOT
        ) *
        frame.atmosphere[2].w +
        CONTRAST_PIVOT,
        vec3f(0.0),
        vec3f(1.0),
    );

    let centered =
        uv * 2.0 -
        1.0;

    let radius_sq =
        dot(
            centered,
            centered,
        );

    let vignette =
        smoothstep(
            0.25,
            1.35,
            radius_sq,
        );

    return display *
        (
            1.0 -
            vignette *
            frame.atmosphere[3].y
        );
}
