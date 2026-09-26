//! Colour values a command can apply.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A colouring: a scheme computed from each atom, one uniform colour, or a
/// scalar property through a named ramp.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "scheme", rename_all = "snake_case", deny_unknown_fields)]
pub enum ColorValue {
    /// Conventional element colours.
    Element,
    /// One categorical colour per chain.
    Chain,
    /// One categorical colour per residue.
    Residue,
    /// Colours for helix, strand, turn and coil.
    SecondaryStructure,
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
        match word {
            "element" => Some(Self::Element),
            "chain" => Some(Self::Chain),
            "residue" => Some(Self::Residue),
            "secondary_structure" => Some(Self::SecondaryStructure),
            other => crate::registry::named_color(other).map(|[red, green, blue]| Self::Rgb {
                red,
                green,
                blue,
            }),
        }
    }

    /// The declarative colour, for every value but a property colour, which
    /// needs the scene's property binding.
    #[must_use]
    pub fn scheme(&self) -> Option<molgfx_api::ColorSpec> {
        Some(match self {
            Self::Element => molgfx_api::ColorSpec::Element,
            Self::Chain => molgfx_api::ColorSpec::Chain,
            Self::Residue => molgfx_api::ColorSpec::Residue,
            Self::SecondaryStructure => molgfx_api::ColorSpec::SecondaryStructure,
            Self::Rgb { red, green, blue } => molgfx_api::ColorSpec::Uniform {
                color: molgfx_api::Color::rgb(*red, *green, *blue),
            },
            Self::Property { .. } => return None,
        })
    }
}

impl fmt::Display for ColorValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Element => formatter.write_str("element"),
            Self::Chain => formatter.write_str("chain"),
            Self::Residue => formatter.write_str("residue"),
            Self::SecondaryStructure => formatter.write_str("secondary_structure"),
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
