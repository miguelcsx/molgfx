//! Reversible monotonic property mappings.

#[cfg(test)]
#[path = "mapping_tests.rs"]
mod tests;

/// A linear property-to-visual mapping with an analytic inverse.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PropertyMapping {
    domain: [f32; 2],
    visual: [f32; 2],
}

/// Invalid mapping endpoints.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum MappingError {
    /// An endpoint was non-finite or either interval had zero extent.
    #[error("mapping intervals must be finite and non-degenerate")]
    InvalidInterval,
}

impl PropertyMapping {
    /// Creates a monotonic increasing or decreasing mapping.
    ///
    /// # Errors
    ///
    /// Returns [`MappingError::InvalidInterval`] for non-finite or degenerate
    /// intervals.
    pub fn new(domain: [f32; 2], visual: [f32; 2]) -> Result<Self, MappingError> {
        let finite = domain.into_iter().chain(visual).all(f32::is_finite);
        if !finite
            || (domain[0] - domain[1]).abs() < f32::EPSILON
            || (visual[0] - visual[1]).abs() < f32::EPSILON
        {
            return Err(MappingError::InvalidInterval);
        }
        Ok(Self { domain, visual })
    }

    /// Maps and clamps a scientific value into the visual interval.
    #[must_use]
    pub fn map(self, value: f32) -> f32 {
        interpolate(value, self.domain, self.visual)
    }

    /// Inverts and clamps a visual value back into scientific units.
    #[must_use]
    pub fn unmap(self, value: f32) -> f32 {
        interpolate(value, self.visual, self.domain)
    }
}

fn interpolate(value: f32, from: [f32; 2], to: [f32; 2]) -> f32 {
    let parameter = ((value - from[0]) / (from[1] - from[0])).clamp(0.0, 1.0);
    to[0] + parameter * (to[1] - to[0])
}
