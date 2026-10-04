//! Cross-section shape and scaling for the ribbon profiles.
//!
//! Each profile answers one question — what shape is the ribbon's cross-section
//! at this sample, and how wide and thick is it — and the answer is what
//! distinguishes a cartoon from a tube from a rocket from a twister. Keeping
//! them together makes them readable side by side.

use super::ribbon::{RibbonParams, SplineProfile};
use super::traces::GuideKind;
use molgfx_core::{CartoonProfile, SecondaryStructure, TubeRadiusMapping};
use molgfx_math::Rgba8;

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
const HELIX_HALF_WIDTH: f32 = 1.35;
const STRAND_HALF_WIDTH: f32 = 1.4;
const LOOP_RADIUS: f32 = 0.2;

/// Body width of a strand relative to the reference half width.
const STRAND_WIDTH_SCALE: f32 = STRAND_HALF_WIDTH / REFERENCE_HALF_WIDTH;

/// Width of a beta-strand at one spline sample.
///
/// Non-terminal strands retain their body width. A terminal strand widens into
/// the arrow shoulder and then narrows towards its tip.
#[inline]
fn strand_width(
    parameter: f32,
    styles: &[SecondaryStructure],
    segment: usize,
    arrow_factor: f32,
) -> f32 {
    let next_style = segment.checked_add(1).and_then(|next| styles.get(next));

    if matches!(next_style, Some(SecondaryStructure::Strand))
        || next_style.is_none()
        || arrow_factor == 0.0
    {
        return STRAND_WIDTH_SCALE;
    }

    arrow_factor * (1.0 - parameter) * STRAND_WIDTH_SCALE
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
    arrow_factor: f32,
) -> (f32, f32) {
    match style {
        state if state.is_helix() => (1.35, 1.35),
        SecondaryStructure::Strand => {
            (strand_width(parameter, styles, segment, arrow_factor), 0.34)
        }
        _ => (0.3, 0.3),
    }
}

#[inline]
pub(super) fn profile_scale(
    style: SecondaryStructure,
    parameter: f32,
    styles: &[SecondaryStructure],
    segment: usize,
    aspect_ratio: f32,
    arrow_factor: f32,
) -> (f32, f32) {
    match style {
        state if state.is_helix() => (
            HELIX_HALF_WIDTH / REFERENCE_HALF_WIDTH,
            HELIX_HALF_WIDTH / aspect_ratio / REFERENCE_HALF_THICKNESS,
        ),
        SecondaryStructure::Strand => (
            strand_width(parameter, styles, segment, arrow_factor),
            STRAND_HALF_WIDTH / aspect_ratio / REFERENCE_HALF_THICKNESS,
        ),
        _ => (
            LOOP_RADIUS / REFERENCE_HALF_WIDTH,
            LOOP_RADIUS / REFERENCE_HALF_THICKNESS,
        ),
    }
}

/// Physical half extents shared by the swept shell and its end faces.
pub(super) fn profile_extents(
    params: RibbonParams,
    style: SecondaryStructure,
    parameter: f32,
    styles: &[SecondaryStructure],
    segment: usize,
) -> (f32, f32) {
    let (width, thickness) = match params.profile {
        SplineProfile::Cartoon => profile_scale(
            style,
            parameter,
            styles,
            segment,
            params.aspect_ratio,
            params.arrow_factor,
        ),
        SplineProfile::Rocket => {
            rocket_scale(style, parameter, styles, segment, params.arrow_factor)
        }
        SplineProfile::Tube | SplineProfile::Twister => (1.0, 1.0),
    };
    let depth = if params.profile == SplineProfile::Cartoon
        && (style.is_helix() || style == SecondaryStructure::Strand)
    {
        params.width.abs() * 0.5 * REFERENCE_HALF_THICKNESS / REFERENCE_HALF_WIDTH * thickness
    } else {
        params.thickness.abs() * 0.5 * thickness
    };
    (params.width.abs() * 0.5 * width, depth)
}

/// Resolves an authored profile against the source guide and spline family.
pub(super) fn sample_profile(
    params: RibbonParams,
    style: SecondaryStructure,
    guide: Option<GuideKind>,
) -> CartoonProfile {
    match params.profile {
        SplineProfile::Twister => CartoonProfile::Square,
        SplineProfile::Cartoon if guide == Some(GuideKind::SugarCarbon) => params.nucleic_profile,
        SplineProfile::Cartoon if style.is_helix() => params.helix_profile,
        SplineProfile::Cartoon | SplineProfile::Rocket | SplineProfile::Tube => {
            CartoonProfile::Elliptical
        }
    }
}

/// Protein and nucleic backbones share authored aspect and width semantics.
pub(super) fn profile_extents_for_guide(
    params: RibbonParams,
    style: SecondaryStructure,
    parameter: f32,
    styles: &[SecondaryStructure],
    segment: usize,
    guide: Option<GuideKind>,
) -> (f32, f32) {
    if params.profile == SplineProfile::Cartoon && guide == Some(GuideKind::SugarCarbon) {
        let width = params.width.abs() * 0.5;
        (width, width / params.aspect_ratio)
    } else {
        profile_extents(params, style, parameter, styles, segment)
    }
}

/// Retains both the strand body and arrow shoulder at their shared guide.
/// Expands reusable sample storage in one reverse pass rather than repeatedly
/// inserting into the middle of a trace.
pub(super) fn arrow_shoulders(
    styles: &[SecondaryStructure],
    samples: &mut Vec<molgfx_math::CurveSample>,
) {
    let shoulder = |sample: &molgfx_math::CurveSample| {
        let Ok(segment) = usize::try_from(sample.segment) else {
            return false;
        };
        sample.parameter.to_bits() == 1.0_f32.to_bits()
            && styles.get(segment + 1) == Some(&SecondaryStructure::Strand)
            && styles
                .get(segment + 2)
                .is_some_and(|style| *style != SecondaryStructure::Strand)
    };
    let count = samples.iter().filter(|sample| shoulder(sample)).count();
    if count == 0 {
        return;
    }
    let length = samples.len();
    let Some(&last) = samples.last() else {
        return;
    };
    samples.resize(length + count, last);
    let mut output = samples.len();
    for input in (0..length).rev() {
        let sample = samples[input];
        if shoulder(&sample) {
            output -= 1;
            samples[output] = molgfx_math::CurveSample {
                segment: sample.segment + 1,
                parameter: 0.0,
                ..sample
            };
        }
        output -= 1;
        samples[output] = sample;
    }
}

#[cfg(test)]
#[path = "profiles_tests.rs"]
mod tests;
