// Nearest-label sampling and style resolution.
//
// Labels are integers, so they are fetched with rounded texel loads and never
// filtered: an interpolated label would name a category that does not exist.
// Unstyled labels resolve to fully transparent rather than to a default hue.

/// Fast in-bounds nearest-label load for the ray-march hot path.
fn label_at(
    coordinate: vec3f,
) -> u32 {
    return segment_label_texel(
        vec3i(
            round(coordinate)
        )
    );
}

/// Bounded variant used only for finite-difference normals.
fn label_at_clamped(
    coordinate: vec3f,
) -> u32 {
    return segment_label_texel(
        vec3i(
            clamp(
                round(coordinate),
                vec3f(0.0),
                vec3f(
                    volume.dimensions.xyz -
                    vec3u(1u)
                ),
            )
        )
    );
}

fn hash_label(label: u32) -> u32 {
    var hash =
        label * 747796405u +
        2891336453u;

    hash =
        (
            (
                hash >>
                ((hash >> 28u) + 4u)
            ) ^
            hash
        ) * 277803737u;

    return (
        hash >> 22u
    ) ^ hash;
}

fn absent_style() -> SegmentStyleSample {
    return SegmentStyleSample(
        vec3f(0.0),
        0.0,
        false,
    );
}

/// Resolves a label using one pipeline-specialized lookup strategy.
fn sample_style(
    label: u32,
) -> SegmentStyleSample {
    let count =
        volume.lookup.y;

    if count == 0u {
        return absent_style();
    }

    if !SEGMENTATION_HASH_LOOKUP {
        if label > volume.lookup.z {
            return absent_style();
        }

        let entry =
            segment_styles[label];

        if entry.present == 0u ||
            entry.label != label {
            return absent_style();
        }

        return SegmentStyleSample(
            entry.color.rgb,
            entry.opacity,
            true,
        );
    }

    let mask =
        count - 1u;

    var index =
        hash_label(label) &
        mask;

    for (var probe = 0u; probe < count; probe++) {
        let entry =
            segment_styles[index];

        if entry.present == 0u {
            return absent_style();
        }

        if entry.label == label {
            return SegmentStyleSample(
                entry.color.rgb,
                entry.opacity,
                true,
            );
        }

        index =
            (index + 1u) &
            mask;
    }

    return absent_style();
