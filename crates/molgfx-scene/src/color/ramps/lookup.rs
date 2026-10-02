//! Finding a ramp by name, reversed or not.

use super::{NamedRamp, diverging, sequential, spectral};
use crate::Color;

/// Every catalogued ramp, sequential first.
pub(super) fn catalogue() -> impl Iterator<Item = &'static NamedRamp> {
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
pub(super) const REVERSED: &str = "_r";

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
