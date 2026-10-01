//! Declarative caller-authored planar guide regions.

use crate::{Color, Error, SceneSpec, StructureId};
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "planes_tests.rs"]
mod tests;
/// A finite rectangular plane outlined by four analytic guide segments.
///
/// The renderer deliberately emits an outline rather than a translucent fill:
/// it reuses the stable guide draw/pick path without introducing a second quad
/// material or ambiguous order-dependent transparency.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct PlaneSpec {
    /// Structure placement that owns the model-space coordinates.
    pub structure: StructureId,
    /// Rectangle center in model-space ångström.
    pub center: [f32; 3],
    /// Surface normal; normalized during lowering.
    pub normal: [f32; 3],
    /// Approximate in-plane tangent; orthogonalized during lowering.
    pub tangent: [f32; 3],
    /// Full dimensions along tangent and bitangent in ångström.
    pub size: [f32; 2],
    /// Outline color.
    pub color: Color,
    /// Pixel-stable outline width.
    #[serde(default = "default_width_pixels")]
    pub width_pixels: f32,
    /// Final outline opacity in `[0, 1]`.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

impl PlaneSpec {
    /// Creates a rectangular plane with the default guide style.
    #[must_use]
    pub fn new(
        structure: StructureId,
        center: [f32; 3],
        normal: [f32; 3],
        tangent: [f32; 3],
        size: [f32; 2],
    ) -> Self {
        Self {
            structure,
            center,
            normal,
            tangent,
            size,
            color: Color::rgb(226, 232, 240),
            width_pixels: default_width_pixels(),
            opacity: default_opacity(),
        }
    }

    pub(crate) fn validate(&self, scene: &SceneSpec) -> Result<(), Error> {
        if !scene.structures.contains_key(&self.structure) {
            return Err(Error::InvalidSpec(
                "plane targets an unknown structure".to_owned(),
            ));
        }
        molgfx_core::PlanarRegion::validate_geometry(
            molgfx_math::Vec3::from_array(self.center),
            molgfx_math::Vec3::from_array(self.normal),
            molgfx_math::Vec3::from_array(self.tangent),
            self.size,
        )
        .map_err(|error| Error::InvalidSpec(error.to_string()))?;
        if !self.width_pixels.is_finite()
            || self.width_pixels <= 0.0
            || !self.opacity.is_finite()
            || !(0.0..=1.0).contains(&self.opacity)
        {
            return Err(Error::InvalidSpec(
                "plane geometry and style must be finite and non-degenerate".to_owned(),
            ));
        }
        Ok(())
    }
}

fn default_width_pixels() -> f32 {
    1.6
}

fn default_opacity() -> f32 {
    1.0
}
