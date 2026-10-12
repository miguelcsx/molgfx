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

impl Legend {
    /// The colour the legend gives `value`: the stops read as the piecewise
    /// linear ramp the renderer paints with, clamped beyond the ends. A value
    /// that is not finite, or stops that cannot form a ramp, give `missing`.
    #[must_use]
    pub fn color_at(&self, value: f32) -> Color {
        let values: Vec<f32> = self.stops.iter().map(|stop| stop.value).collect();
        let colors: Vec<_> = self.stops.iter().map(|stop| stop.color.native()).collect();
        let Ok(ramp) = molgfx_core::ScalarRamp::new(&values, &colors) else {
            return self.missing;
        };
        let sampled = ramp.sample(value, self.missing.native());
        Color([sampled.r, sampled.g, sampled.b, sampled.a])
    }
}
