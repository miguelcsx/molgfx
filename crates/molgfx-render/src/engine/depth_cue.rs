//! Explicit view-space depth-cue settings.

use super::profile_numeric::{finite_clamp, lerp, unit};
use serde::{Deserialize, Serialize};

/// Distance-based atmospheric cue applied in view space after lighting.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct DepthCue {
    /// View-space distance where the cue begins.
    pub near_distance: f32,
    /// View-space distance where the cue reaches full strength.
    pub far_distance: f32,
    /// Blend toward the resolved backdrop, in `[0, 1]`.
    pub strength: f32,
}

impl DepthCue {
    pub(super) fn sanitize(self) -> Self {
        let near_distance = finite_clamp(self.near_distance, 0.0, 999_999.0, 0.0);
        let minimum_far = near_distance + near_distance.mul_add(1.0e-6, 1.0e-3);
        let far_distance = finite_clamp(
            self.far_distance,
            minimum_far,
            1_000_000.0,
            minimum_far.max(1.0),
        );
        Self {
            near_distance,
            far_distance,
            strength: unit(self.strength),
        }
    }

    pub(super) fn blend(self, other: Self, weight: f32) -> Self {
        let weight = unit(weight);
        Self {
            near_distance: lerp(self.near_distance, other.near_distance, weight),
            far_distance: lerp(self.far_distance, other.far_distance, weight),
            strength: lerp(self.strength, other.strength, weight),
        }
        .sanitize()
    }

    pub(crate) fn packed(self) -> [f32; 4] {
        let cue = self.sanitize();
        [cue.near_distance, cue.far_distance, cue.strength, 0.0]
    }
}
