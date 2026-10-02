//! Lookup of named palettes and ramps.

use super::{Color, ramps};

/// Resolves a named ramp to its anchor colours. An unknown name is an error, not
/// a silent default: quietly substituting one palette for another produces a
/// figure whose colors do not mean what its legend says they mean.
pub(crate) fn ramp_colors(name: &str) -> Result<Vec<Color>, crate::Error> {
    let Some((ramp, reversed)) = ramps::lookup(name) else {
        let known = ramps::names().join(", ");
        return Err(crate::Error::InvalidSpec(format!(
            "unknown color ramp '{name}'; known ramps are {known} (append _r to reverse one)"
        )));
    };
    Ok(ramp.colors(reversed))
}

/// The names of every categorical palette.
#[must_use]
pub fn palette_names() -> Vec<&'static str> {
    molgfx_core::CategoryPalette::ALL
        .map(molgfx_core::CategoryPalette::name)
        .to_vec()
}

/// A ramp name without its `_r` reversing suffix.
#[must_use]
pub fn ramp_base_name(name: &str) -> &str {
    ramps::base_name(name)
}

/// The names of every catalogued ramp; each also exists reversed with `_r`.
#[must_use]
pub fn ramp_names() -> Vec<&'static str> {
    ramps::names()
}

/// `index / last` as a fraction, saturating for absurd stop counts.
pub(crate) fn index_fraction(index: usize, last: usize) -> f32 {
    let numerator = u16::try_from(index).map_or(1.0, f32::from);
    let denominator = u16::try_from(last).map_or(1.0, f32::from);
    numerator / denominator
}
