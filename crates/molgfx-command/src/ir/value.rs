//! Validated scalar values that commands carry.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A finite, strictly positive length or size.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Positive(f32);

impl Positive {
    /// Validates `value`.
    ///
    /// # Errors
    ///
    /// Returns a description when `value` is not finite and positive.
    pub fn new(value: f32) -> Result<Self, &'static str> {
        if value.is_finite() && value > 0.0 {
            Ok(Self(value))
        } else {
            Err("must be a finite number greater than zero")
        }
    }

    /// The value.
    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for Positive {
    type Error = &'static str;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Positive> for f32 {
    fn from(value: Positive) -> Self {
        value.0
    }
}

impl fmt::Display for Positive {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A finite number of either sign.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Finite(f32);

impl Finite {
    /// Validates `value`.
    ///
    /// # Errors
    ///
    /// Returns a description when `value` is not finite.
    pub fn new(value: f32) -> Result<Self, &'static str> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err("must be a finite number")
        }
    }

    /// The value.
    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for Finite {
    type Error = &'static str;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Finite> for f32 {
    fn from(value: Finite) -> Self {
        value.0
    }
}

impl fmt::Display for Finite {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// An opacity between zero (invisible) and one (opaque), inclusive.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Opacity(f32);

impl Opacity {
    /// Validates `value`.
    ///
    /// # Errors
    ///
    /// Returns a description when `value` is outside zero to one.
    pub fn new(value: f32) -> Result<Self, &'static str> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err("must be a number from 0 to 1")
        }
    }

    /// The value.
    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for Opacity {
    type Error = &'static str;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Opacity> for f32 {
    fn from(value: Opacity) -> Self {
        value.0
    }
}

impl fmt::Display for Opacity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}
