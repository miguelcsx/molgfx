//! A structure a scene declares and how it is placed.

use serde::{Deserialize, Serialize};

/// Portable description of where molecular data can be resolved.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct StructureSource {
    /// SHA-256 identity of the bound coordinate snapshot.
    pub content_hash: Box<str>,
    /// Optional portable origin.
    pub uri: Option<Box<str>>,
    /// Optional format hint.
    pub format: Option<Box<str>>,
    /// Where this copy of the source sits in the world, as a column-major 4×4
    /// affine matrix; absent places it at the origin frame of its coordinates.
    ///
    /// Several structures may carry the same `content_hash` with different
    /// placements, which is how an assembly's copies are described without
    /// repeating the coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<[f32; 16]>,
}

impl StructureSource {
    /// Checks that a placement is a finite, invertible, rigid-or-affine
    /// transform: a bottom row of `0 0 0 1` and a non-zero determinant.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error naming the failed condition.
    pub fn validate_placement(matrix: &[f32; 16]) -> Result<(), crate::Error> {
        if matrix.iter().any(|value| !value.is_finite()) {
            return Err(crate::Error::InvalidSpec(
                "a structure placement must be finite".to_owned(),
            ));
        }
        let bottom = [matrix[3], matrix[7], matrix[11], matrix[15]];
        if bottom.map(f32::to_bits) != [0.0_f32, 0.0, 0.0, 1.0].map(f32::to_bits) {
            return Err(crate::Error::InvalidSpec(
                "a structure placement must be an affine transform with bottom row 0 0 0 1"
                    .to_owned(),
            ));
        }
        if molgfx_math::Mat4::from_cols_array(matrix)
            .determinant()
            .abs()
            <= f32::EPSILON
        {
            return Err(crate::Error::InvalidSpec(
                "a structure placement must be invertible".to_owned(),
            ));
        }
        Ok(())
    }
}
