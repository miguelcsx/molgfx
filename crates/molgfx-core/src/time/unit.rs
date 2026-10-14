//! Units of physical time and their exact scale in canonical seconds.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[cfg(test)]
#[path = "unit_tests.rs"]
mod tests;

/// A unit of physical time, from attoseconds to seconds.
///
/// The derived ordering runs from the shortest unit to the longest, so
/// `TimeUnit::ALL` is sorted and a display can pick the largest unit that does
/// not exceed a magnitude.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TimeUnit {
    /// Attoseconds, the scale of electronic motion.
    Attosecond,
    /// Femtoseconds, the period of the fastest bond vibrations.
    Femtosecond,
    /// Picoseconds, the native unit of molecular-dynamics time stamps.
    Picosecond,
    /// Nanoseconds, the scale of side-chain and loop motion.
    Nanosecond,
    /// Microseconds, the scale of domain motion and folding intermediates.
    Microsecond,
    /// Milliseconds, the scale of slow conformational change.
    Millisecond,
    /// Seconds, the canonical SI unit.
    Second,
}

impl TimeUnit {
    /// Every unit, ordered from shortest to longest.
    pub const ALL: [Self; 7] = [
        Self::Attosecond,
        Self::Femtosecond,
        Self::Picosecond,
        Self::Nanosecond,
        Self::Microsecond,
        Self::Millisecond,
        Self::Second,
    ];

    /// The largest unit that does not exceed `seconds` in magnitude.
    ///
    /// Zero and NaN read in seconds; a magnitude below one femtosecond reads in
    /// femtoseconds.
    #[must_use]
    pub fn fitting(seconds: f64) -> Self {
        let magnitude = seconds.abs();
        if magnitude == 0.0 || magnitude.is_nan() {
            return Self::Second;
        }
        // `ALL` ascends, so the last unit the magnitude reaches is the largest.
        Self::ALL
            .into_iter()
            .filter(|unit| magnitude >= unit.seconds())
            .fold(Self::Femtosecond, |_, unit| unit)
    }

    /// Length of one unit in canonical seconds.
    #[must_use]
    pub const fn seconds(self) -> f64 {
        match self {
            Self::Attosecond => 1.0e-18,
            Self::Femtosecond => 1.0e-15,
            Self::Picosecond => 1.0e-12,
            Self::Nanosecond => 1.0e-9,
            Self::Microsecond => 1.0e-6,
            Self::Millisecond => 1.0e-3,
            Self::Second => 1.0,
        }
    }

    /// Conventional symbol, from `as` through `s`.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Attosecond => "as",
            Self::Femtosecond => "fs",
            Self::Picosecond => "ps",
            Self::Nanosecond => "ns",
            Self::Microsecond => "µs",
            Self::Millisecond => "ms",
            Self::Second => "s",
        }
    }

    /// Parses a unit symbol, accepting `us` for microseconds.
    ///
    /// Returns `None` for anything else, so an unrecognised unit is an error at
    /// the boundary rather than a silent fallback to seconds.
    #[must_use]
    pub fn from_symbol(symbol: &str) -> Option<Self> {
        match symbol {
            "as" => Some(Self::Attosecond),
            "fs" => Some(Self::Femtosecond),
            "ps" => Some(Self::Picosecond),
            "ns" => Some(Self::Nanosecond),
            "µs" | "us" => Some(Self::Microsecond),
            "ms" => Some(Self::Millisecond),
            "s" => Some(Self::Second),
            _ => None,
        }
    }
}

impl Serialize for TimeUnit {
    /// Writes the unit as its symbol.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.symbol())
    }
}

impl<'de> Deserialize<'de> for TimeUnit {
    /// Reads a unit symbol; any other string is a parse error.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let symbol = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
        Self::from_symbol(&symbol).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "unknown time unit `{symbol}`; expected one of as, fs, ps, ns, us, ms, s"
            ))
        })
    }
}
