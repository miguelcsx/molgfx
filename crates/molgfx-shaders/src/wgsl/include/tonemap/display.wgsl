// Tone curves, gamut mapping and display transfer encoding.
//
// The curve and the transfer function are chosen from the caller's declared
// display rather than assumed, so an SDR, HLG and PQ target each receive the
// encoding it expects instead of a gamma approximation of one.

fn aces_fitted(
    color: vec3f,
) -> vec3f {
    const A: f32 = 2.51;
    const B: f32 = 0.03;
    const C: f32 = 2.43;
    const D: f32 = 0.59;
    const E: f32 = 0.14;

    let numerator =
        color *
        (A * color + B);

    let denominator =
        color *
        (C * color + D) +
        E;

    return clamp(
        numerator /
            denominator,
        vec3f(0.0),
        vec3f(1.0),
    );
}

fn reinhard(
    color: vec3f,
) -> vec3f {
    let positive =
        max(
            color,
            vec3f(0.0),
        );

    return positive /
        (vec3f(1.0) + positive);
}

fn tone_map(
    color: vec3f,
) -> vec3f {
    let tone_tag =
        frame.atmosphere[3].z;

    if tone_tag < 0.5 {
        return aces_fitted(color);
    }

    if tone_tag < 1.5 {
        return reinhard(color);
    }

    return max(
        color,
        vec3f(0.0),
    );
}

fn display_gamut_tagged(
    color: vec3f,
    gamut_tag: f32,
) -> vec3f {
    if gamut_tag < 0.5 {
        return color;
    }

    if gamut_tag < 1.5 {
        return vec3f(
            0.8225927 * color.r
                + 0.1775330 * color.g,

            0.0331990 * color.r
                + 0.9667835 * color.g,

            0.0170853 * color.r
                + 0.0723957 * color.g
                + 0.9103015 * color.b,
        );
    }

    return vec3f(
        0.627404 * color.r
            + 0.329283 * color.g
            + 0.043313 * color.b,

        0.069097 * color.r
            + 0.919540 * color.g
            + 0.011362 * color.b,

        0.016391 * color.r
            + 0.088013 * color.g
            + 0.895595 * color.b,
    );
}

fn display_gamut(
    color: vec3f,
) -> vec3f {
    return display_gamut_tagged(
        color,
        PRESENTATION_GAMUT_TAG,
    );
}

fn pq_encode(
    color: vec3f,
) -> vec3f {
    let normalized =
        max(
            color,
            vec3f(0.0),
        ) *
        (
            frame.atmosphere[5].w *
            PQ_INV_MAX_NITS
        );

    let powered =
        pow(
            normalized,
            vec3f(PQ_M1),
        );

    let ratio =
        (
            vec3f(PQ_C1) +
            vec3f(PQ_C2) *
                powered
        ) /
        (
            vec3f(1.0) +
            vec3f(PQ_C3) *
                powered
        );

    return pow(
        ratio,
        vec3f(PQ_M2),
    );
}

fn hlg_encode_channel(
    value: f32,
) -> f32 {
    let positive =
        max(
            value,
            0.0,
        );

    if positive <= HLG_THRESHOLD {
        return sqrt(
            3.0 *
            positive,
        );
    }

    return HLG_A *
        log(
            max(
                12.0 * positive -
                    HLG_B,
                HLG_LOG_EPSILON,
            ),
        ) +
        HLG_C;
}

fn hlg_encode(
    color: vec3f,
) -> vec3f {
    return vec3f(
        hlg_encode_channel(color.r),
        hlg_encode_channel(color.g),
        hlg_encode_channel(color.b),
    );
}

fn linear_to_srgb_channel(
    value: f32,
) -> f32 {
    if value <= SRGB_THRESHOLD {
        return value * 12.92;
    }

    return 1.055 *
        pow(
            value,
            SRGB_POWER,
        ) -
        0.055;
}

fn linear_to_srgb(
    color: vec3f,
) -> vec3f {
    return vec3f(
        linear_to_srgb_channel(color.r),
        linear_to_srgb_channel(color.g),
        linear_to_srgb_channel(color.b),
    );
}

fn encode_transfer_tagged(
    color: vec3f,
    transfer_tag: f32,
) -> vec3f {
    if transfer_tag < 0.5 {
        return linear_to_srgb(color);
    }

    if transfer_tag < 1.5 {
        return max(
            color,
            vec3f(0.0),
        );
    }

    if transfer_tag < 2.5 {
        return pq_encode(color);
    }

    return hlg_encode(color);
}

fn encode_transfer(
    color: vec3f,
) -> vec3f {
    return encode_transfer_tagged(
        color,
        PRESENTATION_TRANSFER_TAG,
    );
}
