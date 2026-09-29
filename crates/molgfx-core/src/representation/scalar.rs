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

/// Physical meaning attached to a scalar grid.
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

/// Most stops a scalar ramp holds.
///
/// Sixteen anchors reproduce every published continuous palette to within one
/// eight-bit step once interpolated, while keeping the ramp a small `Copy`
/// value the renderer can compare and upload without allocation.
pub const MAX_RAMP_STOPS: usize = 16;

/// A piecewise-linear scalar ramp of two to [`MAX_RAMP_STOPS`] stops whose
/// numeric domain is always available to a legend. Values outside the domain
/// clamp to the nearest endpoint.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScalarRamp {
    len: u8,
    values: [f32; MAX_RAMP_STOPS],
    colors: [Rgba8; MAX_RAMP_STOPS],
}

impl ScalarRamp {
    /// Creates a strictly increasing finite ramp.
    ///
    /// # Errors
    ///
    /// There must be two to [`MAX_RAMP_STOPS`] stops, one colour per value, and
    /// the values must be finite and strictly increasing.
    pub fn new(values: &[f32], colors: &[Rgba8]) -> Result<Self, CoreError> {
        let len = values.len();
        if !(2..=MAX_RAMP_STOPS).contains(&len) || colors.len() != len {
            return Err(CoreError::InvalidVolume {
                reason: "a scalar ramp needs two to sixteen stops with one colour each",
            });
        }
        if values.iter().any(|value| !value.is_finite())
            || values.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(CoreError::InvalidVolume {
                reason: "scalar ramp values must be finite and strictly increasing",
            });
        }
        let Ok(count) = u8::try_from(len) else {
            return Err(CoreError::InvalidVolume {
                reason: "scalar ramp has too many stops",
            });
        };
        let mut ramp = Self {
            len: count,
            values: [0.0; MAX_RAMP_STOPS],
            colors: [Rgba8::WHITE; MAX_RAMP_STOPS],
        };
        ramp.values[..len].copy_from_slice(values);
        ramp.colors[..len].copy_from_slice(colors);
        Ok(ramp)
    }

    /// Spreads `colors` evenly over `domain`.
    ///
    /// # Errors
    ///
    /// The domain must be finite and increasing, and there must be two to
    /// [`MAX_RAMP_STOPS`] colours.
    pub fn evenly(domain: [f32; 2], colors: &[Rgba8]) -> Result<Self, CoreError> {
        let count = colors.len();
        if !(2..=MAX_RAMP_STOPS).contains(&count)
            || domain[0].partial_cmp(&domain[1]) != Some(std::cmp::Ordering::Less)
        {
            return Err(CoreError::InvalidVolume {
                reason: "an even ramp needs an increasing domain and two to sixteen colours",
            });
        }
        let mut values = [0.0_f32; MAX_RAMP_STOPS];
        let last = count - 1;
        for (index, value) in values.iter_mut().take(count).enumerate() {
            let fraction = u16::try_from(index).map_or(1.0, f32::from)
                / u16::try_from(last).map_or(1.0, f32::from);
            *value = domain[0] + (domain[1] - domain[0]) * fraction;
        }
        // Pin the endpoints so rounding cannot leave the last stop short.
        values[last] = domain[1];
        Self::new(&values[..count], colors)
    }

    /// Symmetric blue-white-red ramp around zero.
    #[must_use]
    pub fn diverging(extent: f32) -> Self {
        let extent = if extent.is_finite() && extent > 0.0 {
            extent
        } else {
            1.0
        };
        Self::three(
            [-extent, 0.0, extent],
            [
                Rgba8::opaque(49, 54, 149),
                Rgba8::opaque(247, 247, 247),
                Rgba8::opaque(165, 0, 38),
            ],
        )
    }

    /// Viridis-class sequential ramp over one caller domain.
    #[must_use]
    pub fn sequential(domain: [f32; 2]) -> Self {
        let [low, high] = finite_domain(domain);
        Self::three(
            [low, low.midpoint(high), high],
            [
                Rgba8::opaque(68, 1, 84),
                Rgba8::opaque(33, 145, 140),
                Rgba8::opaque(253, 231, 37),
            ],
        )
    }

    /// A three-stop ramp; the values are the caller's to keep increasing.
    fn three(values: [f32; 3], colors: [Rgba8; 3]) -> Self {
        let mut ramp = Self {
            len: 3,
            values: [0.0; MAX_RAMP_STOPS],
            colors: [Rgba8::WHITE; MAX_RAMP_STOPS],
        };
        ramp.values[..3].copy_from_slice(&values);
        ramp.colors[..3].copy_from_slice(&colors);
        ramp
    }

    /// Samples the piecewise-linear ramp; NaN resolves to `missing`.
    ///
    /// Binary search over the stops: `O(log n)`, at most five comparisons.
    #[must_use]
    pub fn sample(self, value: f32, missing: Rgba8) -> Rgba8 {
        if !value.is_finite() {
            return missing;
        }
        let values = self.values();
        let colors = self.colors();
        let upper = values.partition_point(|stop| *stop < value);
        if upper == 0 {
            return colors[0];
        }
        if upper >= values.len() {
            return colors[values.len() - 1];
        }
        let (from, to) = (values[upper - 1], values[upper]);
        let parameter = ((value - from) / (to - from)).clamp(0.0, 1.0);
        mix_color(colors[upper - 1], colors[upper], parameter)
    }

    /// Number of stops.
    #[must_use]
    pub const fn len(self) -> usize {
        self.len as usize
    }

    /// A ramp always has at least two stops.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        false
    }

    /// Numeric stop values in ascending order.
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values[..usize::from(self.len)]
    }

    /// Colors corresponding exactly to [`Self::values`].
    #[must_use]
    pub fn colors(&self) -> &[Rgba8] {
        &self.colors[..usize::from(self.len)]
    }

    /// Lowest and highest stop value.
    #[must_use]
    pub fn domain(&self) -> [f32; 2] {
        let values = self.values();
        [values[0], values[values.len() - 1]]
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
