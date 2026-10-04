//! Affine placement invariants shared by scalar and categorical grids.

use molgfx_math::Mat4;

pub(super) fn is_valid(transform: Mat4) -> bool {
    let determinant = transform.determinant();
    transform
        .to_cols_array()
        .iter()
        .all(|component| component.is_finite())
        && determinant.is_finite()
        && determinant.abs() > 1e-12
        && transform.w_axis.w.to_bits() == 1.0_f32.to_bits()
        && transform.x_axis.w == 0.0
        && transform.y_axis.w == 0.0
        && transform.z_axis.w == 0.0
        && transform
            .inverse()
            .to_cols_array()
            .iter()
            .all(|component| component.is_finite())
}
