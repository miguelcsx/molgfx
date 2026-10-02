//! Colour values a command can apply.

use molgfx_scene::color::{AtomCategory, AtomMetric};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A colouring: a scheme computed from each atom, one uniform colour, or a
/// scalar property through a named ramp.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "scheme", rename_all = "snake_case", deny_unknown_fields)]
pub enum ColorValue {
    /// Conventional element colours.
    Element,
    /// A category the structure defines — chain, entity, molecule type,
    /// residue name, residue, secondary structure — through a palette.
    Category {
        /// What atoms are categorised by.
        by: AtomCategory,
        /// Palette name; the category's own default when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        palette: Option<Box<str>>,
        /// Whether only carbon atoms take a colour.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        carbon_only: bool,
    },
    /// A value the structure defines for itself, through a ramp.
    Metric {
        /// Which value.
        metric: AtomMetric,
        /// Ramp name; the metric's own default when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ramp: Option<Box<str>>,
        /// Explicit domain; the metric's own default when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        domain: Option<[f32; 2]>,
    },
    /// One colour everywhere, in sRGB.
    Rgb {
        /// Red channel.
        red: u8,
        /// Green channel.
        green: u8,
        /// Blue channel.
        blue: u8,
    },
    /// A scalar property bound to the scene, mapped through a named ramp.
    ///
    /// Only a whole layer can take a property colour: the ramp's domain and
    /// legend describe one representation.
    Property {
        /// The bound property's name.
        property: Box<str>,
        /// Ramp name, such as `viridis`.
        ramp: Box<str>,
        /// Explicit domain; the property's own declared domain when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        domain: Option<[f32; 2]>,
    },
}

/// The scheme word for carbon atoms coloured by chain.
pub(crate) const CARBON_BY_CHAIN: &str = "carbon_by_chain";

impl ColorValue {
    /// Parses `#rrggbb`.
    ///
    /// # Errors
    ///
    /// Returns a description of what is wrong with the literal.
    pub fn hex(text: &str) -> Result<Self, String> {
        let Some(digits) = text.strip_prefix('#') else {
            return Err("a hexadecimal colour starts with '#'".to_owned());
        };
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!(
                "'{text}' is not a colour; write six hexadecimal digits, as in #ff3366"
            ));
        }
        let channel = |range: std::ops::Range<usize>| {
            digits
                .get(range)
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
        };
        match (channel(0..2), channel(2..4), channel(4..6)) {
            (Some(red), Some(green), Some(blue)) => Ok(Self::Rgb { red, green, blue }),
            _ => Err(format!("'{text}' is not a colour")),
        }
    }

    /// The colour or scheme a word names: a scheme name or one of the few
    /// colour names listed by [`crate::registry`].
    #[must_use]
    pub fn named(word: &str) -> Option<Self> {
        if word == "element" {
            return Some(Self::Element);
        }
        if word == CARBON_BY_CHAIN {
            return Some(Self::Category {
                by: AtomCategory::Chain,
                palette: None,
                carbon_only: true,
            });
        }
        if let Some(by) = AtomCategory::from_name(word) {
            return Some(Self::Category {
                by,
                palette: None,
                carbon_only: false,
            });
        }
        if let Some(metric) = AtomMetric::from_name(word) {
            return Some(Self::Metric {
                metric,
                ramp: None,
                domain: None,
            });
        }
        crate::registry::named_color(word).map(|[red, green, blue]| Self::Rgb { red, green, blue })
    }

    /// The declarative colour, for every value but a property colour, which
    /// needs the scene's property binding.
    #[must_use]
    pub fn scheme(&self) -> Option<molgfx_scene::ColorSpec> {
        Some(match self {
            Self::Element => molgfx_scene::ColorSpec::Element,
            Self::Category {
                by,
                palette,
                carbon_only,
            } => molgfx_scene::ColorSpec::Category {
                by: *by,
                palette: palette.clone(),
                carbon_only: *carbon_only,
            },
            Self::Metric {
                metric,
                ramp,
                domain,
            } => molgfx_scene::ColorSpec::Metric {
                metric: *metric,
                ramp: ramp.clone(),
                domain: *domain,
            },
            Self::Rgb { red, green, blue } => molgfx_scene::ColorSpec::Uniform {
                color: molgfx_scene::Color::rgb(*red, *green, *blue),
            },
            Self::Property { .. } => return None,
        })
    }
}

impl fmt::Display for ColorValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Element => formatter.write_str("element"),
            Self::Category {
                by,
                palette,
                carbon_only,
            } => {
                let word = if *carbon_only && *by == AtomCategory::Chain {
                    CARBON_BY_CHAIN
                } else {
                    by.name()
                };
                formatter.write_str(word)?;
                if let Some(palette) = palette {
                    write!(formatter, " palette={palette}")?;
                }
                Ok(())
            }
            Self::Metric {
                metric,
                ramp,
                domain,
            } => {
                formatter.write_str(metric.name())?;
                if let Some(ramp) = ramp {
                    write!(formatter, " ramp={ramp}")?;
                }
                if let Some([low, high]) = domain {
                    write!(formatter, " domain={low}:{high}")?;
                }
                Ok(())
            }
            Self::Rgb { red, green, blue } => write!(formatter, "#{red:02x}{green:02x}{blue:02x}"),
            Self::Property {
                property,
                ramp,
                domain,
            } => {
                write!(formatter, "property {property} ramp={ramp}")?;
                if let Some([low, high]) = domain {
                    write!(formatter, " domain={low}:{high}")?;
                }
                Ok(())
            }
        }
    }
}
