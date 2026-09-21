//! Immutable scientific color specifications.

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

/// Portable scientific legend generated from a color specification.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Legend {
    /// Human-readable property name.
    pub title: Box<str>,
    /// Optional scientific units.
    pub units: Option<Box<str>>,
    /// Ordered samples spanning the explicit domain.
    pub stops: Vec<LegendStop>,
    /// Color for missing values.
    pub missing: Color,
}

/// Declarative coloring rule evaluated from shared molecular metadata.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ColorSpec {
    /// Conventional element colors.
    #[default]
    Element,
    /// Stable categorical colors by chain.
    Chain,
    /// Stable categorical colors by residue.
    Residue,
    /// Colors for unknown, coil, helix, strand and turn states.
    SecondaryStructure,
    /// One constant color.
    Uniform {
        /// Constant color for every selected entity.
        color: Color,
    },
    /// Scalar property mapped through a named scientific ramp.
    Property {
        /// Property name resolved from the molecular source.
        name: Box<str>,
        /// Palette name such as `viridis`, `plasma` or `coolwarm`.
        ramp: Box<str>,
        /// Explicit numeric domain.
        domain: [f32; 2],
        /// Scientific units displayed by the legend.
        units: Option<Box<str>>,
        /// Color assigned to unavailable values.
        missing: Color,
    },
}

impl ColorSpec {
    pub(crate) fn validate(&self) -> Result<(), crate::Error> {
        if let Self::Property {
            name, ramp, domain, ..
        } = self
            && (name.trim().is_empty()
                || ramp.trim().is_empty()
                || !domain[0].is_finite()
                || !domain[1].is_finite()
                || domain[0] >= domain[1])
        {
            return Err(crate::Error::InvalidSpec(
                "property colors require names and a finite increasing domain".to_owned(),
            ));
        }
        Ok(())
    }

    /// Generates a compact deterministic legend for scalar property colors.
    #[must_use]
    pub fn legend(&self) -> Option<Legend> {
        let Self::Property {
            name,
            ramp,
            domain,
            units,
            missing,
        } = self
        else {
            return None;
        };
        let colors = palette(ramp);
        let middle = domain[0] + (domain[1] - domain[0]) * 0.5;
        Some(Legend {
            title: name.clone(),
            units: units.clone(),
            stops: vec![
                LegendStop {
                    value: domain[0],
                    color: colors[0],
                },
                LegendStop {
                    value: middle,
                    color: colors[1],
                },
                LegendStop {
                    value: domain[1],
                    color: colors[2],
                },
            ],
            missing: *missing,
        })
    }

    pub(crate) fn native(&self) -> Result<molgfx_core::ColorScheme, crate::Error> {
        self.validate()?;
        Ok(match self {
            Self::Element => molgfx_core::ColorScheme::ByElement,
            Self::Chain => molgfx_core::ColorScheme::ByChain,
            Self::Residue => molgfx_core::ColorScheme::ByResidue,
            Self::SecondaryStructure => molgfx_core::ColorScheme::BySecondaryStructure,
            Self::Uniform { color } => molgfx_core::ColorScheme::Uniform(color.native()),
            Self::Property { .. } => {
                return Err(crate::Error::InvalidSpec(
                    "property colors require a scene-bound atom property".to_owned(),
                ));
            }
        })
    }
}

/// Colors by element.
#[must_use]
pub const fn element() -> ColorSpec {
    ColorSpec::Element
}

/// Colors by chain.
#[must_use]
pub const fn chain() -> ColorSpec {
    ColorSpec::Chain
}

/// Stable categorical colors by residue.
#[must_use]
pub const fn residue() -> ColorSpec {
    ColorSpec::Residue
}

/// Colors by secondary-structure state.
#[must_use]
pub const fn secondary_structure() -> ColorSpec {
    ColorSpec::SecondaryStructure
}

/// Uses one color everywhere.
#[must_use]
pub const fn uniform(color: Color) -> ColorSpec {
    ColorSpec::Uniform { color }
}

/// Maps a scalar molecular property through an explicit scientific domain.
#[must_use]
pub fn property(
    name: impl Into<Box<str>>,
    ramp: impl Into<Box<str>>,
    domain: [f32; 2],
    units: Option<Box<str>>,
    missing: Color,
) -> ColorSpec {
    ColorSpec::Property {
        name: name.into(),
        ramp: ramp.into(),
        domain,
        units,
        missing,
    }
}

pub(crate) fn palette(name: &str) -> [Color; 3] {
    match name {
        "plasma" => [
            Color::rgb(13, 8, 135),
            Color::rgb(203, 71, 120),
            Color::rgb(240, 249, 33),
        ],
        "coolwarm" => [
            Color::rgb(59, 76, 192),
            Color::rgb(221, 221, 221),
            Color::rgb(180, 4, 38),
        ],
        _ => [
            Color::rgb(68, 1, 84),
            Color::rgb(33, 145, 140),
            Color::rgb(253, 231, 37),
        ],
    }
}
