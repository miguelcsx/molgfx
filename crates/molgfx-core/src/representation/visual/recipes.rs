//! Concise recipes for common scientific visual mappings.

use super::{VisualError, VisualProgramBuilder, VisualStyle};
use crate::{AtomPropertyHandle, ScalarRamp};
use molgfx_math::Rgba8;

impl VisualStyle {
    /// Colors a drawable from one caller property through a reversible ramp.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when a ramp stop is not finite. The instruction
    /// and property budgets cannot be exhausted by a recipe this small.
    pub fn color_by_property(
        property: AtomPropertyHandle,
        ramp: ScalarRamp,
    ) -> Result<Self, VisualError> {
        let mut builder = VisualProgramBuilder::new();
        let value = builder.atom_property(property)?;
        let color = builder.ramp(value, ramp)?;
        builder.set_base_color(color)?;
        builder.finish().map(Self::new)
    }

    /// Uses one property for color and bounded emission.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when a ramp stop or the emission strength is not
    /// finite.
    pub fn emissive_property(
        property: AtomPropertyHandle,
        ramp: ScalarRamp,
        emission_strength: f32,
    ) -> Result<Self, VisualError> {
        let mut builder = VisualProgramBuilder::new();
        let value = builder.atom_property(property)?;
        let color = builder.ramp(value, ramp)?;
        let strength = builder.scalar(emission_strength)?;
        let black = builder.color([0.0, 0.0, 0.0, 1.0])?;
        let emission = builder.mix_color(black, color, strength)?;
        builder.set_base_color(color)?;
        builder.set_emission(emission)?;
        builder.finish().map(Self::new)
    }

    /// Periodically modulates emission without host callbacks.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::InvalidRange`] when a bound is not finite, the
    /// rate is negative, or the minimum exceeds the maximum.
    pub fn pulse(
        color: Rgba8,
        cycles_per_second: f32,
        minimum: f32,
        maximum: f32,
    ) -> Result<Self, VisualError> {
        if !cycles_per_second.is_finite()
            || cycles_per_second < 0.0
            || !minimum.is_finite()
            || !maximum.is_finite()
            || minimum < 0.0
            || minimum > maximum
        {
            return Err(VisualError::InvalidRange);
        }
        let mut builder = VisualProgramBuilder::new();
        let time = builder.time()?;
        let angular_rate = builder.scalar(cycles_per_second * std::f32::consts::TAU)?;
        let phase = builder.multiply(time, angular_rate)?;
        let sine = builder.sine(phase)?;
        let one = builder.scalar(1.0)?;
        let shifted = builder.add(sine, one)?;
        let half = builder.scalar(0.5)?;
        let weight = builder.multiply(shifted, half)?;
        let low = builder.scalar(minimum)?;
        let high = builder.scalar(maximum)?;
        let strength = builder.mix_scalar(low, high, weight)?;
        let black = builder.color([0.0, 0.0, 0.0, 1.0])?;
        let target = builder.color(color.to_f32())?;
        let emission = builder.mix_color(black, target, strength)?;
        builder.set_emission(emission)?;
        builder.finish().map(Self::new)
    }
}
