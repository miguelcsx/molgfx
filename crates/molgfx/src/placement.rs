//! Rigid-body placement matrices.

/// The column-major 4x4 affine matrix of `x' = rotation * x + translation`.
///
/// `rotation` is row-major (`rotation[row][column]`), as a superposition
/// reports it; the result is the layout a scene's placement takes.
#[must_use]
pub fn rigid(rotation: [[f32; 3]; 3], translation: [f32; 3]) -> [f32; 16] {
    let mut out = [0.0_f32; 16];
    for (row, values) in rotation.iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            out[column * 4 + row] = *value;
        }
    }
    out[12..15].copy_from_slice(&translation);
    out[15] = 1.0;
    out
}
