//! A named ramp and how it is defined.

use crate::Color;

/// How a ramp is meant to be read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RampKind {
    /// Low to high, one direction.
    Sequential,
    /// Two directions away from a neutral middle.
    Diverging,
    /// Hue sweep for ordered but unitless positions such as sequence order.
    Spectral,
}

/// One catalogued ramp: its name and anchor colours as `0xRRGGBB`.
#[derive(Clone, Copy, Debug)]
pub struct NamedRamp {
    /// Catalogue name.
    pub name: &'static str,
    /// Reading direction.
    pub kind: RampKind,
    /// Anchor colours, first to last.
    pub anchors: &'static [u32],
}

impl NamedRamp {
    /// The anchors as colours, reversed when `reversed`.
    #[must_use]
    pub fn colors(&self, reversed: bool) -> Vec<Color> {
        let mut colors: Vec<Color> = self.anchors.iter().map(|hex| from_hex(*hex)).collect();
        if reversed {
            colors.reverse();
        }
        colors
    }
}

pub(super) const fn from_hex(hex: u32) -> Color {
    let [_, red, green, blue] = hex.to_be_bytes();
    Color::rgb(red, green, blue)
}
