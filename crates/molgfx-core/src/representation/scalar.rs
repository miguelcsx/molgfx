//! Semantics and reversible presentation for caller-supplied scalar fields.
//!
//! The renderer never derives the values. A calibrated field records its
//! quantity, units and provenance; an uncalibrated field is explicitly a rank.

use crate::{CoreError, VolumeHandle};
use molgfx_math::Rgba8;
use std::sync::Arc;

#[cfg(test)]
#[path = "scalar_tests.rs"]
mod tests;

/// Scientific meaning attached to a scalar grid.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum ScalarFieldSemantics {
    /// Values are useful only for relative ordering and carry no physical unit.
    #[default]
    UncalibratedRank,
    /// A named, unit-bearing quantity with caller-recorded provenance.
    Quantity {
        /// Human-readable quantity such as `electrostatic potential`.
        name: Arc<str>,
        /// Unit symbol such as `kT/e` or `V`.
        units: Arc<str>,
        /// Stable upstream method, dataset or calculation identifier.
        provenance: Arc<str>,
    },
}

impl ScalarFieldSemantics {
    /// Creates calibrated field semantics.
    ///
    /// # Errors
    ///
    /// Every label must contain at least one non-whitespace character.
    pub fn quantity(
        name: Arc<str>,
        units: Arc<str>,
        provenance: Arc<str>,
    ) -> Result<Self, CoreError> {
        if [&name, &units, &provenance]
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(CoreError::InvalidVolume {
                reason: "scalar quantity, units and provenance must be non-empty",
            });
        }
        Ok(Self::Quantity {
            name,
            units,
            provenance,
        })
    }
}

/// A three-stop scalar ramp whose numeric domain is always available to a
/// legend. Values outside the domain clamp to the nearest endpoint.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScalarRamp {
    values: [f32; 3],
    colors: [Rgba8; 3],
}

impl ScalarRamp {
    /// Creates a strictly increasing finite ramp.
    ///
    /// # Errors
    ///
    /// Stop values must be finite and strictly increasing.
    pub fn new(values: [f32; 3], colors: [Rgba8; 3]) -> Result<Self, CoreError> {
        if values.iter().any(|value| !value.is_finite())
            || values[0] >= values[1]
            || values[1] >= values[2]
        {
            return Err(CoreError::InvalidVolume {
                reason: "scalar ramp values must be finite and strictly increasing",
            });
        }
        Ok(Self { values, colors })
    }

    /// Symmetric blue-white-red ramp around zero.
    #[must_use]
    pub fn diverging(extent: f32) -> Self {
        let extent = if extent.is_finite() && extent > 0.0 {
            extent
        } else {
            1.0
        };
        Self {
            values: [-extent, 0.0, extent],
            colors: [
                Rgba8::opaque(49, 54, 149),
                Rgba8::opaque(247, 247, 247),
                Rgba8::opaque(165, 0, 38),
            ],
        }
    }

    /// Viridis-class sequential ramp over one caller domain.
    #[must_use]
    pub fn sequential(domain: [f32; 2]) -> Self {
        let [low, high] = finite_domain(domain);
        Self {
            values: [low, low.midpoint(high), high],
            colors: [
                Rgba8::opaque(68, 1, 84),
                Rgba8::opaque(33, 145, 140),
                Rgba8::opaque(253, 231, 37),
            ],
        }
    }

    /// Samples the piecewise-linear ramp; NaN resolves to `missing`.
    #[must_use]
    pub fn sample(self, value: f32, missing: Rgba8) -> Rgba8 {
        if !value.is_finite() {
            return missing;
        }
        let segment = usize::from(value > self.values[1]);
        let from = self.values[segment];
        let to = self.values[segment + 1];
        let parameter = ((value - from) / (to - from)).clamp(0.0, 1.0);
        mix_color(self.colors[segment], self.colors[segment + 1], parameter)
    }

    /// Numeric stop values in ascending order.
    #[must_use]
    pub const fn values(self) -> [f32; 3] {
        self.values
    }

    /// Colors corresponding exactly to [`Self::values`].
    #[must_use]
    pub const fn colors(self) -> [Rgba8; 3] {
        self.colors
    }
}

fn finite_domain(domain: [f32; 2]) -> [f32; 2] {
    if domain[0].is_finite() && domain[1].is_finite() && domain[0] < domain[1] {
        domain
    } else {
        [0.0, 1.0]
    }
}

fn mix_color(from: Rgba8, to: Rgba8, parameter: f32) -> Rgba8 {
    let channel = |from: u8, to: u8| {
        let value = f32::from(from) + (f32::from(to) - f32::from(from)) * parameter;
        molgfx_math::round_u8(value)
    };
    Rgba8::new(
        channel(from.r, to.r),
        channel(from.g, to.g),
        channel(from.b, to.b),
        channel(from.a, to.a),
    )
}

impl Default for ScalarRamp {
    fn default() -> Self {
        Self::diverging(1.0)
    }
}

/// Pixel-stable isocontours over a sampled scalar surface.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScalarContours {
    /// Numeric interval between adjacent contours, in field units.
    pub interval: f32,
    /// Half-width of each contour in physical pixels.
    pub width_pixels: f32,
}

impl ScalarContours {
    /// Validates an isocontour presentation.
    ///
    /// # Errors
    ///
    /// Interval and width must be finite and positive.
    pub fn new(interval: f32, width_pixels: f32) -> Result<Self, CoreError> {
        if !interval.is_finite()
            || interval <= 0.0
            || !width_pixels.is_finite()
            || width_pixels <= 0.0
        {
            return Err(CoreError::InvalidVolume {
                reason: "scalar contour interval and width must be finite and positive",
            });
        }
        Ok(Self {
            interval,
            width_pixels,
        })
    }
}

/// Surface presentation of one resident caller-supplied scalar grid.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SurfaceScalarOverlay {
    /// Grid sampled on the molecular boundary.
    pub field: VolumeHandle,
    /// Reversible scalar-to-colour mapping.
    pub ramp: ScalarRamp,
    /// Optional analytic, derivative-antialiased isocontours.
    pub contours: Option<ScalarContours>,
    /// Sampling displacement along the world-space surface normal, Ångström.
    pub sample_offset_angstrom: f32,
}

impl SurfaceScalarOverlay {
    /// Creates a surface field overlay with no contours or normal offset.
    #[must_use]
    pub const fn new(field: VolumeHandle, ramp: ScalarRamp) -> Self {
        Self {
            field,
            ramp,
            contours: None,
            sample_offset_angstrom: 0.0,
        }
    }
}
