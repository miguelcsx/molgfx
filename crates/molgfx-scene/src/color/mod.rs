//! Immutable color specifications.

mod derived;
pub mod ramps;
mod spec;
#[cfg(test)]
mod tests;

pub use derived::{AtomCategory, AtomMetric, DerivedColumn};
pub use spec::ColorSpec;

pub(crate) use derived::DERIVED_FORMAT;

use molgfx_math::Rgba8;
use serde::{Deserialize, Serialize};

/// A serializable RGBA color.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Color(pub [u8; 4]);

impl Color {
    /// Opaque RGB color.
    #[must_use]
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self([red, green, blue, 255])
    }

    /// Linear floating-point channels suitable for shader literals.
    #[must_use]
    pub fn to_linear_f32(self) -> [f32; 4] {
        [
            srgb_to_linear(self.0[0]),
            srgb_to_linear(self.0[1]),
            srgb_to_linear(self.0[2]),
            f32::from(self.0[3]) / 255.0,
        ]
    }

    pub(crate) const fn native(self) -> Rgba8 {
        Rgba8::new(self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

fn srgb_to_linear(channel: u8) -> f32 {
    let value = f32::from(channel) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// One generated legend stop in data units.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LegendStop {
    /// Scalar value in the declared domain.
    pub value: f32,
    /// Display color at this value.
    pub color: Color,
}

/// Portable legend generated from a color specification.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Legend {
    /// Human-readable property name.
    pub title: Box<str>,
    /// Optional physical units.
    pub units: Option<Box<str>>,
    /// Ordered samples spanning the explicit domain.
    pub stops: Vec<LegendStop>,
    /// Color for missing values.
    pub missing: Color,
}

/// Colors by element.
#[must_use]
pub const fn element() -> ColorSpec {
    ColorSpec::Element
}

/// Colors by chain.
#[must_use]
pub const fn chain() -> ColorSpec {
    ColorSpec::category(AtomCategory::Chain)
}

/// Colors by entity, the distinct molecular species.
#[must_use]
pub const fn entity() -> ColorSpec {
    ColorSpec::category(AtomCategory::Entity)
}

/// Colors by molecule type: water, ion, protein, RNA, DNA, saccharide.
#[must_use]
pub const fn molecule_type() -> ColorSpec {
    ColorSpec::category(AtomCategory::MoleculeType)
}

/// Colors amino acids and nucleotides by chemistry.
#[must_use]
pub const fn residue_name() -> ColorSpec {
    ColorSpec::category(AtomCategory::ResidueName)
}

/// Stable categorical colors by residue.
#[must_use]
pub const fn residue() -> ColorSpec {
    ColorSpec::category(AtomCategory::Residue)
}

/// Colors by secondary-structure state.
#[must_use]
pub const fn secondary_structure() -> ColorSpec {
    ColorSpec::category(AtomCategory::SecondaryStructure)
}

/// Colors only carbon atoms by chain and leaves every other element its own
/// colour, the convention of most molecular viewers.
#[must_use]
pub const fn carbon_by_chain() -> ColorSpec {
    ColorSpec::Category {
        by: AtomCategory::Chain,
        palette: None,
        carbon_only: true,
    }
}

/// Uses one color everywhere.
#[must_use]
pub const fn uniform(color: Color) -> ColorSpec {
    ColorSpec::Uniform { color }
}

/// Colors by a value the structure defines for itself, on that metric's own
/// ramp and domain.
#[must_use]
pub const fn metric(metric: AtomMetric) -> ColorSpec {
    ColorSpec::Metric {
        metric,
        ramp: None,
        domain: None,
    }
}

/// Maps a scalar molecular property through an explicit physical domain.
#[must_use]
pub fn property(
    property: crate::ScalarProperty,
    ramp: impl Into<Box<str>>,
    domain: [f32; 2],
    units: Option<Box<str>>,
    missing: Color,
) -> ColorSpec {
    ColorSpec::Property {
        property,
        ramp: ramp.into(),
        domain,
        units,
        missing,
    }
}

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
