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
    [0.923_879_5, 0.382_683_43],
    [0.707_106_77, 0.707_106_77],
    [0.382_683_43, 0.923_879_5],
    [0.0, 1.0],
    [-0.382_683_43, 0.923_879_5],
    [-0.707_106_77, 0.707_106_77],
    [-0.923_879_5, 0.382_683_43],
    [-1.0, 0.0],
    [-0.923_879_5, -0.382_683_43],
    [-0.707_106_77, -0.707_106_77],
    [-0.382_683_43, -0.923_879_5],
    [0.0, -1.0],
    [0.382_683_43, -0.923_879_5],
    [0.707_106_77, -0.707_106_77],
    [0.923_879_5, -0.382_683_43],
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
    [-1.0, 1.0, -1.0, 0.0],
    [-1.0, -1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0, -1.0],
    [1.0, -1.0, 0.0, -1.0],
    [-1.0, -1.0, 0.0, -1.0],
    [1.0, -1.0, 0.0, -1.0],
    [1.0, -1.0, 1.0, 0.0],
    [1.0, 1.0, 1.0, 0.0],
    [1.0, -1.0, 1.0, 0.0],
    [1.0, 1.0, 1.0, 0.0],
    [1.0, 1.0, 0.0, 1.0],
    [1.0, 1.0, 0.0, 1.0],
];

/// One cross-section vertex: offset coefficients and the normal that belongs
/// with them.
///
/// The rounded profile's normal is the ellipse gradient, which is why width and
/// thickness swap between position and normal. The flat profile reads its
/// normal from the table instead, because a face's normal is a property of the
/// face and not of where the corner sits on it.
#[inline]
pub(super) fn cross_section(
    flat: bool,
    side: usize,
    width: f32,
    thickness: f32,
) -> (f32, f32, [f32; 2]) {
    let side = side % PROFILE_SIDES;

    if flat {
        let [x, y, normal_x, normal_y] = BOX_PROFILE[side];
        return (x, y, [normal_x, normal_y]);
    }

    let [x, y] = PROFILE[side];

    (x, y, [x * thickness, y * width])
}

/// Twister's two-tone shell: each face names which way the ring plane looks.
#[inline]
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
pub(super) const BOX_FACE_SIDES: [bool; PROFILE_SIDES] = [
    true, false, false, false, true, false, false, false, true, false, false, false, true, false,
    false, false,
];

/// The four distinct corners of the flat cross-section, in ring order.
pub(super) const BOX_CORNERS: [[f32; 2]; 4] = [[1.0, 1.0], [-1.0, 1.0], [-1.0, -1.0], [1.0, -1.0]];

/// Reads one finite variable-radius guide value.
///
/// A control outside the addressable property range, or a property carrying a
/// non-finite value, is semantically equivalent to a missing guide.
#[inline]
fn finite_property(properties: &[f32], control: u32) -> Option<f32> {
    let Ok(index) = usize::try_from(control) else {
        return None;
    };

    let &value = properties.get(index)?;

    value.is_finite().then_some(value)
}

/// CPU reference for the variable-radius tube shader.
///
/// Missing guide values select the finite neighbour, or `fallback` when both
/// are absent. The shader implements this same order of operations so focused
/// differential tests can compare mapped radii without raster uncertainty.
#[must_use]
#[inline]
pub fn variable_tube_radius(
    mapping: TubeRadiusMapping,
    properties: &[f32],
    controls: [u32; 2],
    amount: f32,
    fallback: f32,
) -> f32 {
    let left = finite_property(properties, controls[0]);
    let right = finite_property(properties, controls[1]);

    let value = match (left, right) {
        (Some(left), Some(right)) => left + (right - left) * amount,
        (Some(value), None) | (None, Some(value)) => value,
        (None, None) => return fallback,
    };

    mapping.radius(value, fallback)
}

/// Half extents, in ångström, that the profile scales below are relative to:
/// half of the default ribbon width and thickness.
const REFERENCE_HALF_WIDTH: f32 = 0.6;
const REFERENCE_HALF_THICKNESS: f32 = 0.14;

/// Conventional cartoon proportions, as half extents in ångström: a helix is a
/// wide flat oval, a strand a wide slab, and a loop a round cord. Tying every
/// scale to a physical size keeps the three element kinds legible against each
/// other whatever width the caller asks for.
const HELIX_HALF_EXTENT: [f32; 2] = [1.35, 0.25];
const STRAND_HALF_EXTENT: [f32; 2] = [1.4, 0.4];
const LOOP_RADIUS: f32 = 0.2;

/// Body width of a strand relative to the reference half width.
const STRAND_WIDTH_SCALE: f32 = STRAND_HALF_EXTENT[0] / REFERENCE_HALF_WIDTH;

/// Width of a beta-strand at one spline sample.
///
/// Non-terminal strands retain their body width. A terminal strand widens into
/// the arrow shoulder and then narrows towards its tip.
#[inline]
fn strand_width(parameter: f32, styles: &[SecondaryStructure], segment: usize) -> f32 {
    let next_style = segment.checked_add(1).and_then(|next| styles.get(next));

    if matches!(next_style, Some(SecondaryStructure::Strand)) {
        return STRAND_WIDTH_SCALE;
    }

    let arrow = if parameter < 0.65 {
        1.0 + parameter * (0.6 / 0.65)
    } else {
        (1.6 * (1.0 - parameter) / 0.35).max(0.08)
    };

    arrow * STRAND_WIDTH_SCALE
}

/// Cross-section for the rocket profile.
///
/// A helix becomes a round column: equal width and thickness, so it reads as a
/// solid rather than a twisted band. A strand keeps the cartoon's flat arrow,
/// because that is what distinguishes it from a helix at a glance. Coil drops
/// to a thin cord so the elements stand out from what connects them.
#[inline]
pub(super) fn rocket_scale(
    style: SecondaryStructure,
    parameter: f32,
    styles: &[SecondaryStructure],
    segment: usize,
) -> (f32, f32) {
    match style {
        SecondaryStructure::Helix => (1.35, 1.35),
        SecondaryStructure::Strand => (strand_width(parameter, styles, segment), 0.34),
        SecondaryStructure::Turn | SecondaryStructure::Coil | SecondaryStructure::Unknown => {
            (0.3, 0.3)
        }
    }
}

#[inline]
pub(super) fn profile_scale(
    style: SecondaryStructure,
    parameter: f32,
    styles: &[SecondaryStructure],
    segment: usize,
) -> (f32, f32) {
    match style {
        SecondaryStructure::Helix => (
            HELIX_HALF_EXTENT[0] / REFERENCE_HALF_WIDTH,
            HELIX_HALF_EXTENT[1] / REFERENCE_HALF_THICKNESS,
        ),
        SecondaryStructure::Strand => (
            strand_width(parameter, styles, segment),
            STRAND_HALF_EXTENT[1] / REFERENCE_HALF_THICKNESS,
        ),
        SecondaryStructure::Turn | SecondaryStructure::Coil | SecondaryStructure::Unknown => (
            LOOP_RADIUS / REFERENCE_HALF_WIDTH,
            LOOP_RADIUS / REFERENCE_HALF_THICKNESS,
        ),
    }
}
