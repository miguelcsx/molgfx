//! Validated view-space depth cue values.

/// Validated view-space depth cue applied after lighting.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DepthCue {
    near_distance: f32,
    far_distance: f32,
    strength: f32,
}

impl DepthCue {
    /// Creates a cue with distances in scene units and a backdrop blend in `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::InvalidSpec`] for non-finite, negative, out-of-range,
    /// or incorrectly ordered values.
    pub fn new(near_distance: f32, far_distance: f32, strength: f32) -> Result<Self, crate::Error> {
        if !near_distance.is_finite()
            || !far_distance.is_finite()
            || !strength.is_finite()
            || near_distance < 0.0
            || far_distance <= near_distance
            || !(0.0..=1.0).contains(&strength)
            || near_distance > 999_999.0
            || far_distance > 1_000_000.0
            || far_distance < near_distance + near_distance.mul_add(1.0e-6, 1.0e-3)
        {
            return Err(crate::Error::InvalidSpec(
                "depth cue requires finite non-negative near distance, a larger far distance, and strength in [0, 1]"
                    .to_owned(),
            ));
        }
        Ok(Self {
            near_distance,
            far_distance,
            strength,
        })
    }

    /// Distance where the cue begins.
    #[must_use]
    pub const fn near_distance(self) -> f32 {
        self.near_distance
    }

    /// Distance where the cue reaches full strength.
    #[must_use]
    pub const fn far_distance(self) -> f32 {
        self.far_distance
    }

    /// Blend toward the resolved backdrop.
    #[must_use]
    pub const fn strength(self) -> f32 {
        self.strength
    }
}
