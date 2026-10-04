//! Portable categorical grids with a separate reversible style table.

use super::DataSource;
use crate::{Color, Error};
use serde::{Deserialize, Serialize};

/// One label's display state, independent of its immutable voxel membership.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SegmentStyle {
    /// Exact categorical value, including zero when explicitly styled.
    pub label: u32,
    /// Display colour.
    pub color: Color,
    /// Optical opacity in the closed unit interval.
    pub opacity: f32,
    /// Whether this label contributes to rendering and picking.
    pub visible: bool,
}

/// Portable categorical-grid descriptor; labels arrive through a runtime binding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SegmentationSpec {
    /// Closed binary-membership boundaries or exact cell optical integration.
    #[serde(default)]
    pub presentation: molgfx_core::SegmentationPresentation,
    /// Content-addressed grid provenance.
    pub source: DataSource,
    /// Voxel dimensions in x, y, z order.
    pub dimensions: [u32; 3],
    /// Column-major affine voxel-index to world-coordinate transform.
    pub voxel_to_world: [f32; 16],
    /// Independent styles. An unlisted label is transparent.
    pub styles: Vec<SegmentStyle>,
}

impl SegmentationSpec {
    /// Checks descriptor geometry and the complete style table.
    ///
    /// # Errors
    /// Returns an error for invalid provenance, dimensions, affine geometry,
    /// duplicate labels or non-finite/out-of-range opacity.
    pub fn validate(&self) -> Result<(), Error> {
        self.source.validate()?;
        super::volume_spec::validate_affine(self.voxel_to_world)?;
        if self
            .dimensions
            .iter()
            .any(|dimension| !(2..=u32::from(u16::MAX)).contains(dimension))
        {
            return Err(Error::InvalidSpec(
                "segmentation dimensions must be between 2 and 65535".into(),
            ));
        }
        let _ = native_styles(&self.styles)?;
        Ok(())
    }
}

pub(crate) fn native_styles(
    styles: &[SegmentStyle],
) -> Result<molgfx_core::SegmentStyleTable, Error> {
    let mut native = Vec::with_capacity(styles.len());
    for style in styles {
        if !style.opacity.is_finite() || !(0.0..=1.0).contains(&style.opacity) {
            return Err(Error::InvalidSpec(
                "segment opacity must be finite in [0, 1]".into(),
            ));
        }
        native.push(molgfx_core::SegmentStyle::new(
            style.label,
            style.color.native(),
            if style.visible { style.opacity } else { 0.0 },
        ));
    }
    Ok(molgfx_core::SegmentStyleTable::new(&native)?)
}

#[cfg(test)]
#[path = "segmentation_spec_tests.rs"]
mod tests;
