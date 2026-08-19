//! Allocation-free declarations for heterogeneous primitive batches.

use super::{AnisotropicEllipsoid, CarbohydrateSymbol, Particle, PlanarRegion, Primitive};
use crate::{CoreError, StructureHandle};
use pdviewx_math::Rgba8;

impl Primitive {
    /// Declares one analytic displacement ellipsoid for later batch insertion.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPrimitive`] for malformed opacity.
    pub fn ellipsoid(
        owner: StructureHandle,
        value: AnisotropicEllipsoid,
        color: Rgba8,
        opacity: f32,
    ) -> Result<Self, CoreError> {
        validate_opacity(opacity)?;
        Ok(Self::Ellipsoid {
            owner,
            value,
            color,
            opacity,
            visible: true,
        })
    }

    /// Declares one caller-resolved carbohydrate symbol.
    #[must_use]
    pub const fn carbohydrate(value: CarbohydrateSymbol) -> Self {
        Self::Carbohydrate(value)
    }

    /// Declares one filled analytic planar region for batch insertion.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPrimitive`] for malformed opacity.
    pub fn planar(value: PlanarRegion, color: Rgba8, opacity: f32) -> Result<Self, CoreError> {
        validate_opacity(opacity)?;
        Ok(Self::Planar {
            value,
            color,
            opacity,
            visible: true,
        })
    }

    /// Declares one validated particle for batch insertion.
    #[must_use]
    pub const fn particle(value: Particle) -> Self {
        Self::Particle(value)
    }
}

fn validate_opacity(opacity: f32) -> Result<(), CoreError> {
    if opacity.is_finite() && (0.0..=1.0).contains(&opacity) {
        Ok(())
    } else {
        Err(CoreError::InvalidPrimitive {
            reason: "primitive opacity must be finite in [0, 1]",
        })
    }
}
