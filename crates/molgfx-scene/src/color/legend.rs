//! Legends that explain a color mapping.

use super::Color;
use serde::{Deserialize, Serialize};

/// One generated legend stop in data units.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LegendStop {
    /// Scalar value in the declared domain.
    pub value: f32,
    /// Display color at this value.
    pub color: Color,
}

/// Portable legend generated from a color specification.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Legend {
    /// Human-readable property name.
    pub title: Box<str>,
    /// Optional physical units.
    pub units: Option<Box<str>>,
    /// Ordered samples spanning the explicit domain.
    pub stops: Vec<LegendStop>,
    /// Color for missing values.
    pub missing: Color,
}
