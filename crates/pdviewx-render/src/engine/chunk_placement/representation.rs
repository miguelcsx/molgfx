use super::ChunkPlacementError;
use pdviewx_math::Rgba8;

/// Renderer-owned representation supported by the paged chunk path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChunkRepresentation {
    /// Pixel-stable circular points, batched across all resident chunks.
    Points {
        /// Diameter in physical pixels.
        diameter_pixels: f32,
        /// Uniform packed display color.
        color: Rgba8,
    },
    /// Analytic van der Waals spheres derived from provider element identities.
    Spacefill {
        /// Multiplier applied to each element's display radius.
        radius_scale: f32,
        /// Uniform packed display color.
        color: Rgba8,
    },
}

impl ChunkRepresentation {
    /// Creates the points representation after validating its finite diameter.
    ///
    /// # Errors
    ///
    /// Rejects non-finite or non-positive diameters.
    pub fn points(diameter_pixels: f32, color: Rgba8) -> Result<Self, ChunkPlacementError> {
        if !diameter_pixels.is_finite() || diameter_pixels <= 0.0 {
            return Err(ChunkPlacementError::InvalidPointDiameter);
        }
        Ok(Self::Points {
            diameter_pixels,
            color,
        })
    }

    /// Creates a space-filling representation over provider-owned radii.
    ///
    /// # Errors
    ///
    /// Rejects non-finite or non-positive radius scales.
    pub fn spacefill(radius_scale: f32, color: Rgba8) -> Result<Self, ChunkPlacementError> {
        if !radius_scale.is_finite() || radius_scale <= 0.0 {
            return Err(ChunkPlacementError::InvalidRadiusScale);
        }
        Ok(Self::Spacefill {
            radius_scale,
            color,
        })
    }
}
