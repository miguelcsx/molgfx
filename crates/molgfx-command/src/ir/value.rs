//! Validated scalar values that commands carry.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Declares a validated `f32` newtype with its constructor, accessor,
/// conversions, serde bridge and `Display`.
macro_rules! scalar {
    (
        $(#[$meta:meta])*
        $name:ident, $failure:literal, $message:literal, |$value:ident| $valid:expr
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
        #[serde(try_from = "f32", into = "f32")]
        pub struct $name(f32);

        impl $name {
            /// Validates `value`.
            ///
            /// # Errors
            ///
            #[doc = concat!("Returns a description when `value` ", $failure, ".")]
            pub fn new($value: f32) -> Result<Self, &'static str> {
                if $valid {
                    Ok(Self($value))
                } else {
                    Err($message)
                }
            }

            /// The value.
            #[must_use]
            pub const fn get(self) -> f32 {
                self.0
            }
        }

        impl TryFrom<f32> for $name {
            type Error = &'static str;

            fn try_from(value: f32) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for f32 {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}", self.0)
            }
        }
    };
}

scalar!(
    /// A finite, strictly positive length or size.
    Positive,
    "is not finite and positive",
    "must be a finite number greater than zero",
    |value| value.is_finite() && value > 0.0
);

scalar!(
    /// A finite number of either sign.
    Finite,
    "is not finite",
    "must be a finite number",
    |value| value.is_finite()
);

scalar!(
    /// A finite length that may be zero but never negative.
    ///
    /// Zero is meaningful for controls that request an exact, unblended result.
    NonNegative,
    "is not finite and non-negative",
    "must be a finite number zero or greater",
    |value| value.is_finite() && value >= 0.0
);

scalar!(
    /// An opacity between zero (invisible) and one (opaque), inclusive.
    Opacity,
    "is outside zero to one",
    "must be a number from 0 to 1",
    |value| value.is_finite() && (0.0..=1.0).contains(&value)
);
