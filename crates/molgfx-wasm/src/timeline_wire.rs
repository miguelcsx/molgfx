//! Host-testable wire rules shared by the timeline and camera-path bindings.
//!
//! Strings and numbers cross the browser boundary; everything that decides what
//! they mean lives here, so the rules are checked natively and the
//! `wasm-bindgen` layer stays a thin adapter. The module names no dependency,
//! so it builds and tests natively.

use std::fmt;

/// Number of `f64` slots one packed reading occupies.
pub const READING_STRIDE: usize = 4;

/// Why a wire value was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireError {
    /// A playback mode name outside `clamp`, `loop`, `pingpong`.
    Mode(String),
    /// A time unit symbol outside the fixed set.
    Unit(String),
    /// An interval that is not exactly `[start, end]`.
    Range(usize),
    /// An easing name outside `linear`, `smoothstep`.
    Easing(String),
    /// The caller's reading buffer cannot hold every track.
    Buffer {
        /// `f64` slots needed.
        needed: usize,
        /// `f64` slots offered.
        offered: usize,
    },
}

impl fmt::Display for WireError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mode(name) => write!(
                out,
                "unknown playback mode `{name}`; expected clamp, loop or pingpong"
            ),
            Self::Unit(symbol) => write!(
                out,
                "unknown time unit `{symbol}`; expected one of as, fs, ps, ns, us, ms, s"
            ),
            Self::Range(count) => write!(
                out,
                "a time range is [start, end] seconds but {count} values arrived"
            ),
            Self::Easing(name) => write!(
                out,
                "unknown camera easing `{name}`; expected linear or smoothstep"
            ),
            Self::Buffer { needed, offered } => write!(
                out,
                "readings buffer holds {offered} values but {needed} are needed"
            ),
        }
    }
}

/// A boundary policy as named on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Hold the first or last sample.
    Clamp,
    /// Wrap from the end back to the start.
    Loop,
    /// Alternate forward and backward.
    PingPong,
}

/// A camera easing as named on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    /// Constant motion.
    Linear,
    /// Zero velocity at segment ends.
    SmoothStep,
}

/// Parses a boundary policy name.
///
/// # Errors
/// Returns [`WireError::Mode`] for any other name.
pub fn parse_mode(name: &str) -> Result<Mode, WireError> {
    match name {
        "clamp" => Ok(Mode::Clamp),
        "loop" => Ok(Mode::Loop),
        "pingpong" => Ok(Mode::PingPong),
        other => Err(WireError::Mode(other.to_owned())),
    }
}

/// Parses a camera easing name.
///
/// # Errors
/// Returns [`WireError::Easing`] for any other name.
pub fn parse_easing(name: &str) -> Result<Easing, WireError> {
    match name {
        "linear" => Ok(Easing::Linear),
        "smoothstep" => Ok(Easing::SmoothStep),
        other => Err(WireError::Easing(other.to_owned())),
    }
}

/// Writes one row of [`READING_STRIDE`] values per reading into `out`.
///
/// `rows` is called twice, once to size and once to fill, because a refused
/// call must write nothing. Nothing is allocated. Returns the number of rows
/// written.
///
/// # Errors
/// Returns [`WireError::Buffer`] when `out` is too small.
pub fn pack_rows<I: Iterator<Item = [f64; READING_STRIDE]>>(
    rows: impl Fn() -> I,
    out: &mut [f64],
) -> Result<usize, WireError> {
    let count = rows().count();
    let needed = count.saturating_mul(READING_STRIDE);
    if out.len() < needed {
        return Err(WireError::Buffer {
            needed,
            offered: out.len(),
        });
    }
    let (slots, _) = out.as_chunks_mut::<READING_STRIDE>();
    for (row, slot) in rows().zip(slots) {
        *slot = row;
    }
    Ok(count)
}

/// Reads an interval given as `[start, end]` seconds.
///
/// # Errors
/// Returns [`WireError::Range`] unless exactly two values arrive.
pub fn parse_range(values: &[f64]) -> Result<[f64; 2], WireError> {
    match *values {
        [start, end] => Ok([start, end]),
        _ => Err(WireError::Range(values.len())),
    }
}
