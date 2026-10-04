//! Validation of authored volume sampling and presentation combinations.

use crate::{CoreError, VolumeRendering, VolumeStyle, VolumeTransferFunction};

impl VolumeStyle {
    pub(crate) fn validate_grid(
        &self,
        dimensions: [u32; 3],
        transform: molgfx_math::Mat4,
    ) -> Result<(), CoreError> {
        if let Some(region) = self.region {
            let _ = crate::VolumeRegion::new(region.minimum(), region.maximum(), dimensions)?;
        }
        if self.rendering != VolumeRendering::Slice {
            let step = transform.minimum_axis_length() * self.step_scale;
            if !step.is_finite() || step < f32::MIN_POSITIVE {
                return Err(invalid(
                    "volume ray step is outside the GPU floating-point range",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        if !self.opacity_scale.is_finite() || self.opacity_scale < 0.0 {
            return Err(invalid(
                "volume opacity scale must be finite and non-negative",
            ));
        }
        if !self.step_scale.is_finite() || self.step_scale <= 0.0 {
            return Err(invalid("volume step scale must be finite and positive"));
        }
        let _ = VolumeTransferFunction::new(self.transfer.points())?;
        if self.rendering == VolumeRendering::Slice && self.slice.is_none() {
            return Err(invalid("slice rendering requires a world-space plane"));
        }
        Ok(())
    }
}

fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidVolume { reason }
}

#[cfg(test)]
#[path = "volume_validation_tests.rs"]
mod tests;
