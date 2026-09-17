//! Declarative filtering for disconnected sampled-surface components.

use thiserror::Error;

#[cfg(test)]
#[path = "surface_components_tests.rs"]
mod tests;

/// Quantity used to decide whether a sampled connected component is retained.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum SurfaceComponentThreshold {
    /// Keep every connected component.
    #[default]
    Disabled,
    /// Minimum exposed voxel-face area in square Angstrom.
    Area(f64),
    /// Minimum occupied voxel volume in cubic Angstrom.
    Volume(f64),
    /// Minimum number of occupied voxels.
    Voxels(u64),
}

/// Validated policy applied to connected components of a sampled isosurface.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct SurfaceComponentPolicy {
    threshold: SurfaceComponentThreshold,
    maximum_components: Option<u32>,
}

impl SurfaceComponentPolicy {
    /// Keeps every sampled component.
    #[must_use]
    pub const fn keep_all() -> Self {
        Self {
            threshold: SurfaceComponentThreshold::Disabled,
            maximum_components: None,
        }
    }

    /// Discards components below an exposed-face area in square Angstrom.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `area` is non-finite or not positive.
    pub fn minimum_area(area: f64) -> Result<Self, SurfaceComponentPolicyError> {
        validate_positive(area, SurfaceComponentMeasure::Area)?;
        Ok(Self {
            threshold: SurfaceComponentThreshold::Area(area),
            maximum_components: None,
        })
    }

    /// Discards components below a voxel volume in cubic Angstrom.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `volume` is non-finite or not positive.
    pub fn minimum_volume(volume: f64) -> Result<Self, SurfaceComponentPolicyError> {
        validate_positive(volume, SurfaceComponentMeasure::Volume)?;
        Ok(Self {
            threshold: SurfaceComponentThreshold::Volume(volume),
            maximum_components: None,
        })
    }

    /// Discards components containing fewer than `voxels` occupied samples.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `voxels` is zero.
    pub const fn minimum_voxels(voxels: u64) -> Result<Self, SurfaceComponentPolicyError> {
        if voxels == 0 {
            return Err(SurfaceComponentPolicyError::ZeroVoxels);
        }
        Ok(Self {
            threshold: SurfaceComponentThreshold::Voxels(voxels),
            maximum_components: None,
        })
    }

    /// Also retains at most the `count` largest qualifying components.
    ///
    /// This limit is consumed by indexed provider meshes. Sampled GPU fields
    /// currently use only the threshold, avoiding a global component sort.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `count` is zero.
    pub const fn with_maximum_components(
        mut self,
        count: u32,
    ) -> Result<Self, SurfaceComponentPolicyError> {
        if count == 0 {
            return Err(SurfaceComponentPolicyError::ZeroMaximumComponents);
        }
        self.maximum_components = Some(count);
        Ok(self)
    }

    /// Validated threshold.
    #[must_use]
    pub const fn threshold(self) -> SurfaceComponentThreshold {
        self.threshold
    }

    /// Optional provider-mesh component ceiling.
    #[must_use]
    pub const fn maximum_components(self) -> Option<u32> {
        self.maximum_components
    }

    /// Whether sampled-field filtering is active.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        !matches!(self.threshold, SurfaceComponentThreshold::Disabled)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SurfaceComponentMeasure {
    Area,
    Volume,
}

/// Invalid declarative connected-component policy.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SurfaceComponentPolicyError {
    /// An area or volume threshold was NaN or infinite.
    #[error("surface component {measure} threshold must be finite")]
    NonFinite {
        /// Quantity being configured.
        measure: &'static str,
    },
    /// An area or volume threshold was zero or negative.
    #[error("surface component {measure} threshold must be strictly positive")]
    NonPositive {
        /// Quantity being configured.
        measure: &'static str,
    },
    /// A voxel threshold of zero cannot hide anything.
    #[error("surface component voxel threshold must be non-zero")]
    ZeroVoxels,
    /// A largest-component ceiling of zero cannot retain a surface.
    #[error("surface component maximum must be non-zero")]
    ZeroMaximumComponents,
}

fn validate_positive(
    value: f64,
    measure: SurfaceComponentMeasure,
) -> Result<(), SurfaceComponentPolicyError> {
    let name = match measure {
        SurfaceComponentMeasure::Area => "area",
        SurfaceComponentMeasure::Volume => "volume",
    };
    if !value.is_finite() {
        return Err(SurfaceComponentPolicyError::NonFinite { measure: name });
    }
    if value <= 0.0 {
        return Err(SurfaceComponentPolicyError::NonPositive { measure: name });
    }
    Ok(())
}
