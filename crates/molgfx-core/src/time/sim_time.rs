//! Physical time points on a simulation clock.

use super::TimeUnit;
use crate::CoreError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[cfg(test)]
#[path = "sim_time_tests.rs"]
mod tests;

/// A physical time point, stored as canonical seconds.
///
/// The unit is applied at the boundary: [`SimTime::from_unit`] reads a value
/// recorded in a given unit, and [`SimTime::to_unit`] or the [`Display`]
/// implementation writes one out. Arithmetic never sees a unit, so a track
/// that spans femtoseconds to milliseconds cannot mix units by accident.
///
/// [`Display`]: fmt::Display
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct SimTime {
    seconds: f64,
}

impl SimTime {
    /// Reads a finite number of canonical seconds.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTimeline`] for NaN or an infinity, so an
    /// unrecorded time stamp cannot enter a track as a number.
    pub fn new(seconds: f64) -> Result<Self, CoreError> {
        if seconds.is_finite() {
            Ok(Self { seconds })
        } else {
            Err(CoreError::InvalidTimeline {
                reason: "simulation time must be finite",
            })
        }
    }

    /// Reads `value` recorded in `unit`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTimeline`] when `value`, or its scaled
    /// seconds, is not finite.
    pub fn from_unit(value: f64, unit: TimeUnit) -> Result<Self, CoreError> {
        Self::new(value * unit.seconds())
    }

    /// Canonical seconds since the simulation origin.
    #[must_use]
    pub const fn seconds(self) -> f64 {
        self.seconds
    }

    /// Whether this time is strictly earlier than `other`.
    ///
    /// Written as an ordering test so an unordered pair, which a NaN produces,
    /// is not earlier: `!a.is_before(b)` is the NaN-aware form of `a >= b`.
    #[must_use]
    pub fn is_before(self, other: Self) -> bool {
        self.partial_cmp(&other) == Some(std::cmp::Ordering::Less)
    }

    /// This time expressed in `unit`.
    #[must_use]
    pub fn to_unit(self, unit: TimeUnit) -> f64 {
        self.seconds / unit.seconds()
    }
}

impl From<f64> for SimTime {
    /// Reads a bare number as canonical seconds without checking it.
    ///
    /// This is a convenience for call sites that hand a number straight to a
    /// frame, segment or warp, each of which rejects a non-finite time itself.
    /// Use [`SimTime::new`] to check at the point of construction.
    fn from(seconds: f64) -> Self {
        Self { seconds }
    }
}

impl fmt::Display for SimTime {
    /// Writes the time in the largest unit whose magnitude it reaches.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = TimeUnit::fitting(self.seconds);
        let value = format!("{:.3}", self.to_unit(unit));
        let trimmed = value.trim_end_matches('0').trim_end_matches('.');
        write!(f, "{trimmed} {}", unit.symbol())
    }
}

impl Serialize for SimTime {
    /// Writes canonical seconds.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.seconds)
    }
}

impl<'de> Deserialize<'de> for SimTime {
    /// Reads canonical seconds and rejects a non-finite value.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let seconds = f64::deserialize(deserializer)?;
        Self::new(seconds).map_err(serde::de::Error::custom)
    }
}
