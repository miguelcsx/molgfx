//! Cross-section shape and scaling for the ribbon profiles.
//!
//! Each profile answers one question — what shape is the ribbon's cross-section
//! at this sample, and how wide and thick is it — and the answer is what
//! distinguishes a cartoon from a tube from a rocket from a twister. Keeping
//! them together makes them readable side by side.

use super::ribbon::{PROFILE_SIDES, SplineProfile};
use molgfx_core::{SecondaryStructure, TubeRadiusMapping};
use molgfx_math::Rgba8;

pub(super) const PROFILE: [[f32; 2]; PROFILE_SIDES] = [
    [1.0, 0.0],
    [0.707_106_77, 0.707_106_77],
    [0.0, 1.0],
    [-0.707_106_77, 0.707_106_77],
    [-1.0, 0.0],
    [-0.707_106_77, -0.707_106_77],
    [0.0, -1.0],
    [0.707_106_77, -0.707_106_77],
];

/// Crisp rectangular cross-section: four flat faces with their own normals.
///
/// The elliptical `PROFILE` derives each normal from the vertex position, so a
/// facet is shaded as if it were curved and the highlight never agrees with the
/// silhouette. A ribbon whose whole job is to show which way a plane faces has
/// to read as flat, so its corners are doubled and each copy carries the normal
/// of the face it belongs to. Traversal order and vertex count match `PROFILE`,
/// which keeps the index topology and the shader's sample shift unchanged; the
/// four zero-area quads between a doubled corner cost nothing to draw.
///
/// Lanes are position x, position y, normal x, normal y.
pub(super) const BOX_PROFILE: [[f32; 4]; PROFILE_SIDES] = [
    [1.0, 1.0, 0.0, 1.0],
    [-1.0, 1.0, 0.0, 1.0],
    [-1.0, 1.0, -1.0, 0.0],
    [-1.0, -1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0, -1.0],
    [1.0, -1.0, 0.0, -1.0],
    [1.0, -1.0, 1.0, 0.0],
    [1.0, 1.0, 1.0, 0.0],
];

/// One cross-section vertex: offset coefficients and the normal that belongs
/// with them.
///
/// The rounded profile's normal is the ellipse gradient, which is why width and
/// thickness swap between position and normal. The flat profile reads its
/// normal from the table instead, because a face's normal is a property of the
/// face and not of where the corner sits on it.
pub(super) fn cross_section(
    flat: bool,
    side: usize,
    width: f32,
    thickness: f32,
) -> (f32, f32, [f32; 2]) {
    if flat {
        let lane = BOX_PROFILE[side % PROFILE_SIDES];
        return (lane[0], lane[1], [lane[2], lane[3]]);
    }
    let [x, y] = PROFILE[side % PROFILE_SIDES];
    (x, y, [x * thickness, y * width])
}

/// Twister's two-tone shell: each face names which way the ring plane looks.
pub(super) fn profile_color(profile: SplineProfile, thickness_axis: f32, base: Rgba8) -> Rgba8 {
    if profile != SplineProfile::Twister {
        return base;
    }
    if thickness_axis > 0.0 {
        Rgba8::new(226, 232, 244, base.a)
    } else {
        Rgba8::new(28, 74, 168, base.a)
    }
}

/// Sides of the flat profile that bound a face.
///
/// The flat profile doubles each corner so the two faces meeting there can carry
/// their own normals, which leaves four sides spanning nothing. A quad whose
/// four corners collapse onto a line is not reliably discarded — the rasterizer
/// can still find a hair of coverage in it — and the sliver it draws takes the
/// colour of whichever face owns that corner, so it appears as a stray fleck of
/// the far face lying on the near one. They are cheaper to leave out than to
/// draw.
pub(super) const BOX_FACE_SIDES: [bool; PROFILE_SIDES] =
    [true, false, true, false, true, false, true, false];

/// The four distinct corners of the flat cross-section, in ring order.
pub(super) const BOX_CORNERS: [[f32; 2]; 4] = [[1.0, 1.0], [-1.0, 1.0], [-1.0, -1.0], [1.0, -1.0]];

/// CPU reference for the variable-radius tube shader.
///
/// Missing guide values select the finite neighbour, or `fallback` when both
/// are absent. The shader implements this same order of operations so focused
/// differential tests can compare mapped radii without raster uncertainty.
#[must_use]
pub fn variable_tube_radius(
    mapping: TubeRadiusMapping,
    properties: &[f32],
    controls: [u32; 2],
    amount: f32,
    fallback: f32,
) -> f32 {
    let left = properties
        .get(usize::try_from(controls[0]).map_or(usize::MAX, |value| value))
        .copied()
        .filter(|value| value.is_finite());
    let right = properties
        .get(usize::try_from(controls[1]).map_or(usize::MAX, |value| value))
        .copied()
        .filter(|value| value.is_finite());
    match left.zip(right) {
        Some((left, right)) => mapping.radius(left + (right - left) * amount, fallback),
        None => left
            .or(right)
            .map_or(fallback, |value| mapping.radius(value, fallback)),
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
        SecondaryStructure::Turn | SecondaryStructure::Coil | SecondaryStructure::Unknown => {
            (0.3, 0.3)
        }
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
        SecondaryStructure::Coil | SecondaryStructure::Unknown => (0.48, 1.0),
    }
}
