//! Reconstruction of a colour scheme from its stable description.

use super::{Scene, invalid, invalid_value, resolve_existing, resolve_raw};
use crate::serialization::types::{self, ColorDescription};
use crate::{AtomPropertyHandle, CategoryPalette, ColorScheme, ScalarRamp};
use molgfx_math::Rgba8;

pub(super) fn parse_color(
    scene: &Scene,
    value: &ColorDescription,
) -> Result<ColorScheme, crate::CoreError> {
    match value.mode.as_str() {
        "element" => Ok(ColorScheme::ByElement),
        "uniform" => Ok(ColorScheme::Uniform(parse_rgba(value.rgba)?)),
        "category" => {
            let property = column_of(
                scene,
                value,
                [
                    "category colour has no property row",
                    "category colour has no property generation",
                ],
            )?;
            let palette = value
                .palette
                .as_deref()
                .and_then(CategoryPalette::from_name)
                .ok_or_else(|| invalid_value("category colour names an unknown palette"))?;
            Ok(ColorScheme::ByCategory { property, palette })
        }
        "property" => {
            let property = column_of(
                scene,
                value,
                [
                    "property colour has no property row",
                    "property colour has no property generation",
                ],
            )?;
            let values: Vec<f32> = value
                .ramp_values
                .as_ref()
                .ok_or_else(|| invalid_value("property colour has no ramp values"))?
                .iter()
                .copied()
                .map(f32::from_bits)
                .collect();
            let colors: Vec<Rgba8> = value
                .ramp_colors
                .as_ref()
                .ok_or_else(|| invalid_value("property colour has no ramp colours"))?
                .iter()
                .copied()
                .map(rgba)
                .collect();
            Ok(ColorScheme::ByProperty {
                property,
                ramp: ScalarRamp::new(&values, &colors)?,
                missing: parse_rgba(value.rgba)?,
            })
        }
        _ => invalid("unknown colour mode"),
    }
}

/// The existing property column a description names.
fn column_of(
    scene: &Scene,
    value: &ColorDescription,
    [missing_row, missing_generation]: [&'static str; 2],
) -> Result<AtomPropertyHandle, crate::CoreError> {
    let row = value
        .property_row
        .ok_or_else(|| invalid_value(missing_row))?;
    let generation = value
        .property_generation
        .ok_or_else(|| invalid_value(missing_generation))?;
    let raw = resolve_raw(types::ObjectIdentity { row, generation });
    resolve_existing(scene.properties.get(raw))?;
    Ok(AtomPropertyHandle(raw))
}

fn parse_rgba(value: Option<[u8; 4]>) -> Result<Rgba8, crate::CoreError> {
    value
        .map(rgba)
        .ok_or_else(|| invalid_value("manifest colour is missing its RGBA value"))
}

pub(super) fn rgba(value: [u8; 4]) -> Rgba8 {
    Rgba8::new(value[0], value[1], value[2], value[3])
}
