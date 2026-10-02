//! The catalogue of named scalar colour ramps.
//!
//! A ramp is a short list of anchor colours spread evenly over the caller's
//! numeric domain and interpolated linearly between anchors. Every name also
//! resolves with an `_r` suffix to the same ramp reversed, so the catalogue
//! stores each ramp once. Lookups are by name and linear in the catalogue size,
//! which is a few dozen entries and is consulted when a colour is authored, not
//! per atom or per frame.

mod confidence;
mod diverging;
mod sequential;
mod spectral;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

mod lookup;
mod named_ramp;

pub(crate) use lookup::default_colors;
pub use lookup::{base_name, lookup, names};
pub use named_ramp::{NamedRamp, RampKind};
