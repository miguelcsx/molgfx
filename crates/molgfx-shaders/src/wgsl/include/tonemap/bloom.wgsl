// Bloom upsample and contribution.
//
// The chain is sampled with an explicit bilinear fetch so the result does not
// depend on a sampler's filtering behaviour, which is what keeps the frame
// reproducible byte for byte across adapters.

fn bloom_bilinear(
    sample_position: vec2f,
    maximum: vec2i,
) -> vec3f {
    let base_f =
        floor(sample_position);

    let fraction =
        sample_position -
        base_f;

    let base =
        vec2i(base_f);

    let p00 = clamp(
        base,
        vec2i(0),
        maximum,
    );

    let p10 = clamp(
        base + vec2i(1, 0),
        vec2i(0),
        maximum,
    );

    let p01 = clamp(
        base + vec2i(0, 1),
        vec2i(0),
        maximum,
    );

    let p11 = clamp(
        base + vec2i(1, 1),
        vec2i(0),
        maximum,
    );

    let c00 =
        textureLoad(
            bloom_texture,
            p00,
            0,
        ).rgb;

    let c10 =
        textureLoad(
            bloom_texture,
            p10,
            0,
        ).rgb;

    let c01 =
        textureLoad(
            bloom_texture,
            p01,
            0,
        ).rgb;

    let c11 =
        textureLoad(
            bloom_texture,
            p11,
            0,
        ).rgb;

    let top =
        mix(
            c00,
            c10,
            fraction.x,
        );

    let bottom =
        mix(
            c01,
            c11,
            fraction.x,
        );

    return mix(
        top,
        bottom,
        fraction.y,
    );
}

/// Quarter-resolution highlight bleed, bilinearly reconstructed at full
/// resolution. Intensity resolves to zero when no profile requests bloom, so
/// the unwritten target contributes nothing.
fn bloom_contribution(
    uv: vec2f,
) -> vec3f {
    let intensity =
        frame.atmosphere[5].y;

    if intensity <= 0.0 {
        return vec3f(0.0);
    }

    let dimensions_i =
        vec2i(
            textureDimensions(
                bloom_texture,
            ),
        );

    let dimensions =
        vec2f(dimensions_i);

    let sample_position =
        uv * dimensions -
        0.5;

    return bloom_bilinear(
        sample_position,
        dimensions_i - 1,
    ) * intensity;
}
