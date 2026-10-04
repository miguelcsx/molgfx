//! Affine density-grid metadata and presentation authoring.

use super::{DataSource, IsoStyle, VolumePresentation};
use crate::{Color, Error};
use molgfx_math::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

/// Portable scalar-grid metadata; values arrive through a runtime binding.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct VolumeSpec {
    /// Grid provenance.
    pub source: DataSource,
    /// Voxel dimensions in x, y, z order.
    pub dimensions: [u32; 3],
    /// Column-major affine mapping from voxel indices to world ångström.
    pub voxel_to_world: [f32; 16],
    /// One to eight independently rendered readings of the same resident grid.
    pub presentations: Vec<VolumePresentation>,
    /// Optional half-open voxel crop.
    pub region: Option<VolumeRegion>,
}

/// Half-open voxel bounds, validated against the descriptor dimensions.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct VolumeRegion {
    /// Inclusive minimum index.
    pub minimum: [u32; 3],
    /// Exclusive maximum index.
    pub maximum: [u32; 3],
}

impl VolumeSpec {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        self.source.validate()?;
        validate_affine(self.voxel_to_world)?;
        if self.dimensions.iter().any(|dimension| *dimension < 2)
            || !(1..=8).contains(&self.presentations.len())
        {
            return Err(Error::InvalidSpec(
                "volume requires at least two voxels per axis and one to eight presentations"
                    .to_owned(),
            ));
        }
        if let Some(region) = self.region {
            let _ =
                molgfx_core::VolumeRegion::new(region.minimum, region.maximum, self.dimensions)?;
        }
        for presentation in &self.presentations {
            presentation.native()?.validate_volume_grid(
                self.dimensions,
                Mat4::from_cols_array(&self.voxel_to_world),
            )?;
        }
        Ok(())
    }

    /// First isosurface's scalar level, if this volume has an isosurface.
    #[must_use]
    pub fn isovalue(&self) -> Option<f32> {
        self.presentations
            .iter()
            .find_map(|presentation| match presentation {
                VolumePresentation::Isosurface { isovalue, .. } => Some(*isovalue),
                _ => None,
            })
    }

    pub(crate) fn set_isovalue(&mut self, value: f32) -> Result<(), Error> {
        if !value.is_finite() {
            return Err(Error::InvalidSpec(
                "volume isovalue must be finite".to_owned(),
            ));
        }
        let level = self
            .presentations
            .iter_mut()
            .find_map(|presentation| match presentation {
                VolumePresentation::Isosurface { isovalue, .. } => Some(isovalue),
                _ => None,
            })
            .ok_or_else(|| {
                Error::InvalidSpec("volume has no isosurface presentation".to_owned())
            })?;
        *level = value;
        Ok(())
    }
}

pub(crate) fn validate_affine(values: [f32; 16]) -> Result<(), Error> {
    let matrix = Mat4::from_cols_array(&values);
    let determinant = matrix.determinant();
    if values.iter().any(|value| !value.is_finite())
        || values[3] != 0.0
        || values[7] != 0.0
        || values[11] != 0.0
        || values[15].to_bits() != 1.0_f32.to_bits()
        || !determinant.is_finite()
        || determinant.abs() <= 1.0e-12
    {
        return Err(Error::InvalidSpec(
            "volume transform must be a finite invertible affine matrix".to_owned(),
        ));
    }
    if !matrix.inverse().is_finite() {
        return Err(Error::InvalidSpec(
            "volume inverse transform must be finite".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn cell_affine(
    origin: [f32; 3],
    spacing: [f32; 3],
    angles: [f32; 3],
) -> Result<[f32; 16], Error> {
    let cell =
        molgfx_core::CrystalCell::new(spacing, angles)?.with_origin(Vec3::from_array(origin))?;
    let origin = cell.origin();
    let matrix = Mat4::from_cols(
        (cell.fractional_to_cartesian([1.0, 0.0, 0.0]) - origin).extend(0.0),
        (cell.fractional_to_cartesian([0.0, 1.0, 0.0]) - origin).extend(0.0),
        (cell.fractional_to_cartesian([0.0, 0.0, 1.0]) - origin).extend(0.0),
        origin.extend(1.0),
    )
    .to_cols_array();
    validate_affine(matrix)?;
    Ok(matrix)
}

/// Immutable density-volume builder. Add presentations explicitly.
#[derive(Clone, PartialEq, Debug)]
pub struct Volume(pub(super) VolumeSpec);

impl Volume {
    /// Sets the column-major voxel-to-world mapping.
    #[must_use]
    pub fn affine(mut self, affine: [f32; 16]) -> Self {
        self.0.voxel_to_world = affine;
        self
    }

    /// Builds an affine voxel basis from cell angles and voxel-axis lengths.
    ///
    /// # Errors
    /// Returns an error for degenerate or non-finite cell geometry.
    pub fn from_cell(
        self,
        origin: [f32; 3],
        spacing: [f32; 3],
        cell_angles_deg: [f32; 3],
    ) -> Result<Self, Error> {
        Ok(self.affine(cell_affine(origin, spacing, cell_angles_deg)?))
    }

    /// Adds one presentation sharing the same grid.
    #[must_use]
    pub fn presentation(mut self, presentation: VolumePresentation) -> Self {
        self.0.presentations.push(presentation);
        self
    }

    /// Adds a solid isosurface.
    #[must_use]
    pub fn isosurface(self, isovalue: f32, color: Color, opacity: f32) -> Self {
        self.presentation(VolumePresentation::Isosurface {
            isovalue,
            color,
            opacity,
            style: IsoStyle::Solid,
        })
    }

    /// Adds a voxel-lattice wire isosurface.
    #[must_use]
    pub fn isosurface_mesh(
        self,
        isovalue: f32,
        color: Color,
        opacity: f32,
        line_width_voxels: f32,
    ) -> Self {
        self.presentation(VolumePresentation::Isosurface {
            isovalue,
            color,
            opacity,
            style: IsoStyle::Mesh { line_width_voxels },
        })
    }

    /// Adds a voxel-lattice dotted isosurface.
    #[must_use]
    pub fn isosurface_dots(
        self,
        isovalue: f32,
        color: Color,
        opacity: f32,
        dot_radius_voxels: f32,
    ) -> Self {
        self.presentation(VolumePresentation::Isosurface {
            isovalue,
            color,
            opacity,
            style: IsoStyle::Dots { dot_radius_voxels },
        })
    }

    /// Adds direct optical integration.
    #[must_use]
    pub fn direct(
        self,
        transfer: Vec<super::VolumeTransferPoint>,
        opacity_scale: f32,
        step_scale: f32,
    ) -> Self {
        self.presentation(VolumePresentation::Direct {
            transfer,
            opacity_scale,
            step_scale,
        })
    }

    /// Adds a world-space transfer-mapped slice using a named color ramp.
    #[must_use]
    pub fn slice(
        self,
        point: [f32; 3],
        normal: [f32; 3],
        ramp: impl Into<Box<str>>,
        domain: [f32; 2],
    ) -> Self {
        self.presentation(VolumePresentation::Slice {
            point,
            normal,
            ramp: ramp.into(),
            domain,
        })
    }

    /// Restricts every presentation to a half-open voxel region.
    #[must_use]
    pub fn region(mut self, minimum: [u32; 3], maximum: [u32; 3]) -> Self {
        self.0.region = Some(VolumeRegion { minimum, maximum });
        self
    }
}

#[cfg(test)]
#[path = "volume_spec_tests.rs"]
mod tests;
