//! Validated camera construction without exposing math implementation types.

/// Builds a perspective look-at camera from plain coordinate triples.
///
/// # Errors
///
/// Returns an invalid-specification error for non-finite or degenerate input.
pub fn perspective(
    position: [f32; 3],
    target: [f32; 3],
    up: [f32; 3],
    fov_y: f32,
    aspect: f32,
    near: f32,
    far: f32,
) -> Result<crate::Camera, crate::Error> {
    let position = molgfx_math::Vec3::from_array(position);
    let target = molgfx_math::Vec3::from_array(target);
    let up = molgfx_math::Vec3::from_array(up);
    let finite = position.is_finite() && target.is_finite() && up.is_finite();
    let valid_projection = fov_y.is_finite()
        && fov_y > 0.0
        && fov_y < std::f32::consts::PI
        && aspect.is_finite()
        && aspect > 0.0
        && near.is_finite()
        && near > 0.0
        && far.is_finite()
        && far > near;
    if !finite
        || position.distance_squared(target) <= f32::EPSILON
        || up.length_squared() <= f32::EPSILON
        || !valid_projection
    {
        return Err(crate::Error::InvalidSpec(
            "camera vectors and perspective parameters must be finite and non-degenerate"
                .to_owned(),
        ));
    }
    Ok(crate::Camera {
        eye: position,
        target,
        up,
        projection: molgfx_math::Projection::Perspective {
            fov_y,
            aspect,
            near,
            far,
        },
    })
}
