//! Scene presentation state shared by timelines and typed visual programs.

use crate::{CoreError, Scene};

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;

impl Scene {
    /// Sets the global presentation clock read by visual-program time inputs.
    ///
    /// This value affects presentation only. It never changes source
    /// coordinates, chemistry, provenance, or reportable scientific values.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTimeline`] when the value is non-finite or
    /// cannot be represented by the portable GPU `f32` clock.
    pub fn set_presentation_time(&mut self, seconds: f64) -> Result<(), CoreError> {
        let value = validate_presentation_time(seconds)?;
        self.apply_presentation_time(value);
        Ok(())
    }

    /// Global presentation time in seconds.
    #[must_use]
    pub const fn presentation_time_seconds(&self) -> f32 {
        self.presentation_time_seconds
    }

    /// Revision bumped only when the presentation clock changes.
    #[must_use]
    pub const fn presentation_revision(&self) -> u64 {
        self.presentation_revision
    }

    pub(crate) fn apply_presentation_time(&mut self, seconds: f32) {
        if self.presentation_time_seconds.to_bits() == seconds.to_bits() {
            return;
        }
        self.presentation_time_seconds = seconds;
        self.presentation_revision = self.presentation_revision.wrapping_add(1);
    }
}

pub(super) fn validate_presentation_time(seconds: f64) -> Result<f32, CoreError> {
    let invalid = || CoreError::InvalidTimeline {
        reason: "presentation time must be a finite portable f32 value",
    };
    if !seconds.is_finite() {
        return Err(invalid());
    }
    let value: f32 = num_traits::cast(seconds).ok_or_else(invalid)?;
    if !value.is_finite() {
        return Err(invalid());
    }
    Ok(value)
}
