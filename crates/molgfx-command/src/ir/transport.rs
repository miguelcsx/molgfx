//! Transport commands: the playback clock and per-track pace.
//!
//! A time is always written with its unit. There is no default unit, because
//! `2` could be two seconds of playback or two nanoseconds of physics, and the
//! wrong guess differs by nine orders of magnitude.

use super::name::Name;
use molgfx_scene::time::TimeUnit;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A finite quantity of time with the unit it was written in.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(try_from = "AmountRepr", into = "AmountRepr")]
pub struct TimeAmount {
    value: f64,
    unit: TimeUnit,
}

#[derive(Serialize, Deserialize)]
struct AmountRepr {
    value: f64,
    unit: TimeUnit,
}

impl TimeAmount {
    /// Validates `value` as a finite number of `unit`.
    ///
    /// # Errors
    ///
    /// Returns a description when `value` is not finite or its seconds
    /// overflow.
    pub fn new(value: f64, unit: TimeUnit) -> Result<Self, &'static str> {
        if value.is_finite() && (value * unit.seconds()).is_finite() {
            Ok(Self { value, unit })
        } else {
            Err("a time must be a finite number")
        }
    }

    /// Reads `text` as a number and `unit` as a unit symbol.
    ///
    /// # Errors
    ///
    /// Returns a description for text that is not a finite number, or a unit
    /// that is not one of `as fs ps ns us ms s`.
    pub fn parse(text: &str, unit: &str) -> Result<Self, String> {
        let value: f64 = text
            .parse()
            .map_err(|_| format!("'{text}' is not a number"))?;
        let unit = TimeUnit::from_symbol(unit).ok_or_else(|| {
            format!("'{unit}' is not a time unit; use as, fs, ps, ns, us, ms or s")
        })?;
        Self::new(value, unit).map_err(str::to_owned)
    }

    /// Zero seconds.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            value: 0.0,
            unit: TimeUnit::Second,
        }
    }

    /// The number as written.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.value
    }

    /// The unit as written.
    #[must_use]
    pub const fn unit(self) -> TimeUnit {
        self.unit
    }

    /// The amount in canonical seconds.
    #[must_use]
    pub fn seconds(self) -> f64 {
        self.value * self.unit.seconds()
    }
}

impl Default for TimeAmount {
    fn default() -> Self {
        Self::zero()
    }
}

impl TryFrom<AmountRepr> for TimeAmount {
    type Error = &'static str;

    fn try_from(repr: AmountRepr) -> Result<Self, Self::Error> {
        Self::new(repr.value, repr.unit)
    }
}

impl From<TimeAmount> for AmountRepr {
    fn from(amount: TimeAmount) -> Self {
        Self {
            value: amount.value,
            unit: amount.unit,
        }
    }
}

impl fmt::Display for TimeAmount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}", self.value, self.unit.symbol())
    }
}

/// One command about time, as carried by `Command::Transport`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transport {
    /// Moves the playback clock to a time. Undoable.
    Seek {
        /// The playback time, in presentation time and never negative.
        time: TimeAmount,
    },
    /// Starts the clock. Transient: not part of history.
    Play,
    /// Stops the clock. Transient: not part of history.
    Pause,
    /// Sets a track's pace to a fixed physical time per playback second.
    /// Undoable.
    Dilate {
        /// The authored timeline track.
        track: Name,
        /// Physical time per playback second, in the unit it was written in.
        rate: TimeAmount,
    },
}

impl Transport {
    /// The verb that writes this command.
    #[must_use]
    pub const fn verb(&self) -> &'static str {
        match self {
            Self::Seek { .. } => "seek",
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Dilate { .. } => "dilate",
        }
    }
}

impl fmt::Display for Transport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Seek { time } => write!(formatter, "seek {time}"),
            Self::Play => formatter.write_str("play"),
            Self::Pause => formatter.write_str("pause"),
            Self::Dilate { track, rate } => write!(formatter, "dilate {track} {rate}"),
        }
    }
}
