// Caller transfer function evaluation.
//
// Colour and opacity come from the caller's control points by interpolation
// only; the renderer neither extrapolates beyond the declared domain nor
// substitutes a default ramp for a supplied one.

// -----------------------------------------------------------------------------
// Transfer function
// -----------------------------------------------------------------------------

fn transfer_count() -> u32 {
    return clamp(
        volume.transfer_meta.x,
        2u,
        8u,
    );
}

fn transfer_amount(
    density: f32,
    low: vec4f,
    high: vec4f,
) -> f32 {
    return clamp(
        (density - low.x) /
        max(
            high.x - low.x,
            VOLUME_RAY_EPSILON,
        ),
        0.0,
        1.0,
    );
}

fn transfer_at(
    density: f32,
    count: u32,
) -> TransferSample {
    if density <
        volume.transfer_values[0].x {
        return TransferSample(
            vec3f(0.0),
            0.0,
        );
    }

    for (var index = 1u; index < count; index++) {
        let low =
            volume.transfer_values[
                index - 1u
            ];

        let high =
            volume.transfer_values[index];

        if density <= high.x {
            let amount =
                transfer_amount(
                    density,
                    low,
                    high,
                );

            return TransferSample(
                mix(
                    volume.transfer_colors[
                        index - 1u
                    ].rgb,
                    volume.transfer_colors[index].rgb,
                    amount,
                ),
                mix(
                    low.y,
                    high.y,
                    amount,
                ),
            );
        }
    }

    return TransferSample(
        volume.transfer_colors[
            count - 1u
        ].rgb,
        volume.transfer_values[
            count - 1u
        ].y,
    );
}

fn transfer_opacity_at(
    density: f32,
    count: u32,
) -> f32 {
    if density <
        volume.transfer_values[0].x {
        return 0.0;
    }

    for (var index = 1u; index < count; index++) {
        let low =
            volume.transfer_values[
                index - 1u
            ];

        let high =
            volume.transfer_values[index];

        if density <= high.x {
            return mix(
                low.y,
                high.y,
                transfer_amount(
                    density,
                    low,
                    high,
                ),
            );
        }
    }

    return volume.transfer_values[
        count - 1u
    ].y;
}

fn transfer_opacity_maximum(
    lower: f32,
    upper: f32,
    count: u32,
) -> f32 {
    var maximum =
        max(
            transfer_opacity_at(
                lower,
                count,
            ),
            transfer_opacity_at(
                upper,
                count,
            ),
        );

    for (var index = 0u; index < count; index++) {
        let value =
            volume.transfer_values[index].x;

        if value >= lower &&
            value <= upper {
            maximum =
                max(
                    maximum,
                    volume.transfer_values[index].y,
                );
        }
    }

    return maximum;
}
