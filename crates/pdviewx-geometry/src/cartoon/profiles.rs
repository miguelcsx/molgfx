//! Cross-section scaling for the ribbon profiles.
//!
//! Each profile answers one question — how wide and how thick is the ribbon at
//! this sample — and the answer is what distinguishes a cartoon from a tube
//! from a rocket. Keeping them together makes the three readable side by side.

use super::ribbon::RibbonParams;
use pdviewx_core::SecondaryStructure;

pub(super) fn putty_radius(
    params: RibbonParams,
    properties: &[f32],
    segment: usize,
    amount: f32,
) -> f32 {
    let fallback = params.width.abs() * 0.5;
    let left = properties
        .get(segment)
        .copied()
        .filter(|value| value.is_finite());
    let right = properties
        .get(segment + 1)
        .copied()
        .filter(|value| value.is_finite());
    match left.zip(right) {
        Some((left, right)) => params
            .radius_mapping
            .radius(left + (right - left) * amount, fallback),
        None => left.or(right).map_or(fallback, |value| {
            params.radius_mapping.radius(value, fallback)
        }),
    }
}

/// Cross-section for the rocket profile.
///
/// A helix becomes a round column: equal width and thickness, so it reads as a
/// solid rather than a twisted band. A strand keeps the cartoon's flat arrow,
/// because that is what distinguishes it from a helix at a glance. Coil drops
/// to a thin cord so the elements stand out from what connects them.
pub(super) fn rocket_scale(
    style: SecondaryStructure,
    parameter: f32,
    styles: &[SecondaryStructure],
    segment: usize,
) -> (f32, f32) {
    match style {
        SecondaryStructure::Helix => (1.35, 1.35),
        SecondaryStructure::Strand => {
            let (width, _) = profile_scale(style, parameter, styles, segment);
            (width, 0.34)
        }
        SecondaryStructure::Turn | SecondaryStructure::Coil => (0.3, 0.3),
    }
}

pub(super) fn profile_scale(
    style: SecondaryStructure,
    parameter: f32,
    styles: &[SecondaryStructure],
    segment: usize,
) -> (f32, f32) {
    match style {
        SecondaryStructure::Helix => (1.0, 1.6),
        SecondaryStructure::Strand => {
            let terminal = !matches!(styles.get(segment + 1), Some(SecondaryStructure::Strand));
            let arrow = if terminal {
                if parameter < 0.65 {
                    1.0 + parameter * (0.6 / 0.65)
                } else {
                    (1.6 * (1.0 - parameter) / 0.35).max(0.08)
                }
            } else {
                1.0
            };
            (arrow * 1.2, 0.58)
        }
        SecondaryStructure::Turn => (0.55, 1.0),
        SecondaryStructure::Coil => (0.48, 1.0),
    }
}
