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

// Continuous membership supports lighting without inventing label identities.
fn segment_membership(coordinate: vec3f, label: u32) -> f32 {
    let point = clamp(coordinate, vec3f(0.0),
        vec3f(volume.dimensions.xyz - vec3u(1u)));
    let lower = vec3i(floor(point));
    let upper = min(lower + vec3i(1), vec3i(volume.dimensions.xyz) - vec3i(1));
    let fraction = fract(point);
    var membership = 0.0;
    for (var corner = 0u; corner < 8u; corner++) {
        let high = vec3<bool>((corner & 1u) != 0u, (corner & 2u) != 0u,
            (corner & 4u) != 0u);
        let texel = select(lower, upper, high);
        let weight = select(vec3f(1.0) - fraction, fraction, high);
        membership += select(0.0, 1.0, segment_label_texel(texel) == label)
            * weight.x * weight.y * weight.z;
    }
    return membership;
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
}
