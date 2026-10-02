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

pub use molgfx_core::{CameraEasing, CameraKeyframe, CameraPath};
pub use molgfx_math::{Ray, ScreenPoint};

/// Where a world point lands on a render target of `size`, in pixels from the
/// top-left, with the renderer's conventions; `None` behind the eye.
#[must_use]
pub fn project(camera: &crate::Camera, point: [f32; 3], size: (u32, u32)) -> Option<ScreenPoint> {
    camera.project(molgfx_math::Vec3::from_array(point), size)
}

/// The ray through pixel `(x, y)` of a render target of `size`; `None` for a
/// degenerate camera.
#[must_use]
pub fn ray(camera: &crate::Camera, x: f32, y: f32, size: (u32, u32)) -> Option<Ray> {
    camera.ray(x, y, size)
}

/// Builds a camera path from `(seconds, camera)` keyframes.
///
/// # Errors
///
/// Returns an invalid-specification error for fewer than two keyframes,
/// timestamps that are not strictly increasing, mixed projection models, or a
/// non-finite or degenerate camera.
pub fn path(
    keyframes: &[(f64, crate::Camera)],
    easing: CameraEasing,
) -> Result<CameraPath, crate::Error> {
    let frames = keyframes
        .iter()
        .map(|(seconds, camera)| {
            CameraKeyframe::new(*seconds, *camera)
                .map_err(|error| crate::Error::InvalidSpec(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    CameraPath::new(frames.into(), easing)
        .map_err(|error| crate::Error::InvalidSpec(error.to_string()))
}

#[cfg(test)]
#[path = "camera_tests.rs"]
mod tests;
