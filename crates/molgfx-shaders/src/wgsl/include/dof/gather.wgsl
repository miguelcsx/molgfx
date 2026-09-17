// Occlusion-aware weighting for one gathered depth-of-field tap.
//
// A tap contributes only where its own circle of confusion actually reaches
// the centre pixel, so a sharp foreground cannot bleed into a blurred
// background. Cost is O(taps) per non-sharp pixel and zero in sharp tiles.

fn dof_sample_coverage_radius(
    sample_coc: f32,
    center_coc: f32,
    distance_sq: f32,
) -> f32 {
    if sample_coc < -SHARP_RADIUS_PIXELS {
        let radius =
            -sample_coc;

        if distance_sq <= radius * radius {
            return radius;
        }

        return 0.0;
    }

    if center_coc > SHARP_RADIUS_PIXELS {
        let radius =
            center_coc;

        if distance_sq <= radius * radius {
            return radius;
        }
    }

    return 0.0;
}

fn dof_coverage_weight(
    coverage_radius: f32,
    distance_pixels: f32,
) -> f32 {
    return 1.0 -
        smoothstep(
            max(
                coverage_radius - 1.0,
                0.0,
            ),
            max(
                coverage_radius,
                1.0,
            ),
            distance_pixels,
        );
}
