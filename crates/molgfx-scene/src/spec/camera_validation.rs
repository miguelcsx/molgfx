//! Validation of an authored camera.

pub(crate) fn validate_camera(camera: Option<molgfx_math::Camera>) -> Result<(), crate::Error> {
    let Some(camera) = camera else {
        return Ok(());
    };
    let vectors_are_finite = [camera.eye, camera.target, camera.up]
        .iter()
        .all(|value| value.is_finite());
    if !vectors_are_finite
        || camera.eye == camera.target
        || camera.up.length_squared() <= f32::EPSILON
        || !camera.projection.aspect().is_finite()
        || camera.projection.aspect() <= 0.0
    {
        return Err(crate::Error::InvalidSpec(
            "camera vectors and projection must be finite and non-degenerate".to_owned(),
        ));
    }
    Ok(())
}
