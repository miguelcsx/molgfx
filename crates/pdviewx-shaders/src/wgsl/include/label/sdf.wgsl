// Analytic stroke, glyph and marker distance fields.
//
// Glyphs are a packed 5x7 bitmask expanded into stroke distances, so text
// stays crisp at any zoom with no font atlas and no texture fetch. Coverage
// comes from the distance directly, giving antialiased edges without
// supersampling the fragment.

fn box_sdf(
    point: vec2f,
    half_extent: vec2f,
) -> f32 {
    let delta =
        abs(point) - half_extent;

    return length(
        max(delta, vec2f(0.0))
    ) + min(
        max(delta.x, delta.y),
        0.0,
    );
}

/// Extracts one 5-bit glyph row from the packed 35-bit bitmap.
fn glyph_row_bits(
    row: u32,
    low: u32,
    high: u32,
) -> u32 {
    if row < 6u {
        return (
            low >> (row * GLYPH_COLUMNS)
        ) & GLYPH_ROW_MASK;
    }

    return (
        (low >> 30u) |
        (high << 2u)
    ) & GLYPH_ROW_MASK;
}

/// Evaluates only occupied glyph cells instead of all 35 cells.
fn glyph_distance(
    point: vec2f,
    low: u32,
    high: u32,
) -> f32 {
    var result = 1.0e6;

    for (
        var row = 0u;
        row < GLYPH_ROWS;
        row++
    ) {
        var bits =
            glyph_row_bits(
                row,
                low,
                high,
            );

        let y =
            (f32(row) - 3.0) *
            GLYPH_STEP.y;

        while bits != 0u {
            let bit =
                firstTrailingBit(bits);

            let center =
                vec2f(
                    (2.0 - f32(bit)) *
                        GLYPH_STEP.x,
                    y,
                );

            result =
                min(
                    result,
                    box_sdf(
                        point - center,
                        GLYPH_HALF_CELL,
                    ),
                );

            // Nothing below this value changes the final opaque result.
            if result <= GLYPH_FILL_INNER {
                return result;
            }

            bits &= bits - 1u;
        }
    }

    return result;
}

/// Computes anti-aliased circular marker coverage while avoiding sqrt()
/// outside the one-pixel transition region.
fn circle_coverage(
    point: vec2f,
    radius: f32,
) -> f32 {
    let distance_sq =
        dot(point, point);

    let inner =
        radius - 0.5;

    let outer =
        radius + 0.75;

    if inner > 0.0
        && distance_sq <= inner * inner {
        return 1.0;
    }

    if distance_sq >= outer * outer {
        return 0.0;
    }

    return 1.0 -
        smoothstep(
            -0.5,
            0.75,
            sqrt(distance_sq) - radius,
        );
}

fn marker_coverage(
    point: vec2f,
    radius: f32,
    shape: u32,
) -> f32 {
    if shape == 1u {
        return 1.0 -
            smoothstep(
                -0.5,
                0.75,
                abs(point.x) +
                    abs(point.y) -
                    radius,
            );
    }

    if shape == 2u {
        let horizontal =
            max(
                abs(point.x) - radius,
                abs(point.y) - 0.8,
            );

        let vertical =
            max(
                abs(point.x) - 0.8,
                abs(point.y) - radius,
            );

        return 1.0 -
            smoothstep(
                -0.5,
                0.75,
                min(horizontal, vertical),
            );
    }

    return circle_coverage(
        point,
        radius,
    );
}
