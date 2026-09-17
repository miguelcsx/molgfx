// Filmic bloom: bright-pass downsample followed by separable Gaussian blur.
//
// Source dimensions are resolved once per fragment. Interior pixels bypass
// coordinate clamps; only border fragments pay for edge handling.

//!include "include/fullscreen.wgsl"
//!include "include/camera.wgsl"

@group(1) @binding(0) var source_texture: texture_2d<f32>;

const BLOOM_AVERAGE_4X4: f32 = 1.0 / 16.0;

const GAUSSIAN_WEIGHTS: array<f32, 5> = array<f32, 5>(
    0.227027,
    0.194595,
    0.121622,
    0.054054,
    0.016216,
);

fn above_threshold(
    color: vec3f,
    threshold: f32,
) -> vec3f {
    let luminance =
        dot(
            color,
            vec3f(0.2126, 0.7152, 0.0722),
        );

    let excess =
        max(
            luminance - threshold,
            0.0,
        );

    return color *
        (
            excess /
            max(luminance, 1.0e-4)
        );
}

@fragment
fn fs_bloom_bright(
    in: FullscreenOut,
) -> @location(0) vec4f {
    let dimensions =
        vec2i(
            textureDimensions(
                source_texture
            )
        );

    let origin =
        vec2i(in.position.xy) * 4;

    var total =
        vec3f(0.0);

    // Almost every 4x4 block is interior and needs no coordinate clamp.
    if all(
        origin + vec2i(3) <
        dimensions
    ) {
        for (var y = 0; y < 4; y++) {
            for (var x = 0; x < 4; x++) {
                total +=
                    textureLoad(
                        source_texture,
                        origin + vec2i(x, y),
                        0,
                    ).rgb;
            }
        }
    } else {
        let maximum =
            dimensions - 1;

        for (var y = 0; y < 4; y++) {
            for (var x = 0; x < 4; x++) {
                total +=
                    textureLoad(
                        source_texture,
                        clamp(
                            origin + vec2i(x, y),
                            vec2i(0),
                            maximum,
                        ),
                        0,
                    ).rgb;
            }
        }
    }

    return vec4f(
        above_threshold(
            total * BLOOM_AVERAGE_4X4,
            frame.atmosphere[5].x,
        ),
        1.0,
    );
}

fn bloom_blur(
    pixel: vec2i,
    direction: vec2i,
) -> vec3f {
    let dimensions =
        vec2i(
            textureDimensions(
                source_texture
            )
        );

    let step =
        max(
            frame.atmosphere[5].z,
            1.0,
        );

    let offsets =
        vec4i(
            round(
                vec4f(1.0, 2.0, 3.0, 4.0) *
                step
            )
        );

    let margin =
        abs(direction) *
        offsets.w;

    let interior =
        all(pixel >= margin) &&
        all(pixel < dimensions - margin);

    var total =
        textureLoad(
            source_texture,
            pixel,
            0,
        ).rgb *
        GAUSSIAN_WEIGHTS[0];

    for (var tap = 0u; tap < 4u; tap++) {
        let delta =
            direction *
            offsets[tap];

        var positive =
            pixel + delta;

        var negative =
            pixel - delta;

        if !interior {
            positive =
                clamp(
                    positive,
                    vec2i(0),
                    dimensions - 1,
                );

            negative =
                clamp(
                    negative,
                    vec2i(0),
                    dimensions - 1,
                );
        }

        let weight =
            GAUSSIAN_WEIGHTS[
                tap + 1u
            ];

        total +=
            (
                textureLoad(
                    source_texture,
                    positive,
                    0,
                ).rgb +
                textureLoad(
                    source_texture,
                    negative,
                    0,
                ).rgb
            ) * weight;
    }

    return total;
}

@fragment
fn fs_bloom_horizontal(
    in: FullscreenOut,
) -> @location(0) vec4f {
    return vec4f(
        bloom_blur(
            vec2i(in.position.xy),
            vec2i(1, 0),
        ),
        1.0,
    );
}

@fragment
fn fs_bloom_vertical(
    in: FullscreenOut,
) -> @location(0) vec4f {
    return vec4f(
        bloom_blur(
            vec2i(in.position.xy),
            vec2i(0, 1),
        ),
        1.0,
    );
}
