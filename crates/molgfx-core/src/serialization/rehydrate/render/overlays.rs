//! Reconstruct source-backed appearance overlays.

use super::{Scene, invalid, resolve_existing, resolve_raw, rgba};
use crate::serialization::types::{self, SurfaceScalarDescription};
use crate::{PropertyAppearance, ScalarContours, ScalarRamp, SurfaceScalarOverlay};
use molgfx_math::Rgba8;

pub(super) fn parse_appearance(
    scene: &Scene,
    value: &types::PropertyAppearanceDescription,
) -> Result<PropertyAppearance, crate::CoreError> {
    let raw = resolve_raw(value.property);
    resolve_existing(scene.properties.get(raw))?;
    PropertyAppearance::new(
        crate::AtomPropertyHandle(raw),
        value.domain,
        value.opacity,
        value.softness_pixels,
        crate::PropertyAppearanceSample {
            opacity: value.missing[0],
            softness_pixels: value.missing[1],
        },
    )
}

pub(super) fn parse_surface_scalar(
    scene: &Scene,
    value: &SurfaceScalarDescription,
) -> Result<SurfaceScalarOverlay, crate::CoreError> {
    let raw = resolve_raw(value.field);
    resolve_existing(scene.volumes.get(raw))?;
    let values: Vec<f32> = value
        .ramp_values
        .iter()
        .copied()
        .map(f32::from_bits)
        .collect();
    let colors: Vec<Rgba8> = value.ramp_colors.iter().copied().map(rgba).collect();
    let ramp = ScalarRamp::new(&values, &colors)?;
    let mut overlay = SurfaceScalarOverlay::new(crate::VolumeHandle(raw), ramp);
    overlay.contours = value
        .contours
        .map(|values| ScalarContours::new(values[0], values[1]))
        .transpose()?;
    if !value.sample_offset_angstrom.is_finite() {
        return invalid("surface scalar offset is not finite");
    }
    overlay.sample_offset_angstrom = value.sample_offset_angstrom;
    Ok(overlay)
}
