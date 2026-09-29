//! The catalogue of named scalar colour ramps.
//!
//! A ramp is a short list of anchor colours spread evenly over the caller's
//! numeric domain and interpolated linearly between anchors. Every name also
//! resolves with an `_r` suffix to the same ramp reversed, so the catalogue
//! stores each ramp once. Lookups are by name and linear in the catalogue size,
//! which is a few dozen entries and is consulted when a colour is authored, not
//! per atom or per frame.

mod diverging;
mod sequential;
mod spectral;

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

const fn from_hex(hex: u32) -> Color {
    let [_, red, green, blue] = hex.to_be_bytes();
    Color::rgb(red, green, blue)
}

/// Every catalogued ramp, sequential first.
fn catalogue() -> impl Iterator<Item = &'static NamedRamp> {
    sequential::RAMPS
        .iter()
        .chain(diverging::RAMPS)
        .chain(spectral::RAMPS)
}

/// The colours legends fall back to when a ramp name is unknown.
pub(crate) fn default_colors() -> Vec<Color> {
    sequential::RAMPS[0].colors(false)
}

/// Suffix that selects a catalogued ramp reversed.
const REVERSED: &str = "_r";

/// Resolves a ramp name, with or without the reversing suffix.
///
/// Returns the ramp and whether to read it backwards.
#[must_use]
pub fn lookup(name: &str) -> Option<(&'static NamedRamp, bool)> {
    if let Some(ramp) = catalogue().find(|ramp| ramp.name == name) {
        return Some((ramp, false));
    }
    let base = name.strip_suffix(REVERSED)?;
    catalogue()
        .find(|ramp| ramp.name == base)
        .map(|ramp| (ramp, true))
}

/// A ramp name without its reversing suffix.
#[must_use]
pub fn base_name(name: &str) -> &str {
    let Some(base) = name.strip_suffix(REVERSED) else {
        return name;
    };
    base
}

/// Every base name in catalogue order; each also exists with `_r`.
#[must_use]
pub fn names() -> Vec<&'static str> {
    catalogue().map(|ramp| ramp.name).collect()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
