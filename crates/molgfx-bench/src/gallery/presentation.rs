//! Authored gallery cameras and quality profiles.
use super::catalog::{Camera, FitCamera, Fixture, Result};
use molgfx::{Scene, profile, profile::MeasuredOutput};
use serde_json::Value;
use std::io;

/// The explicit camera of a case at the catalog's aspect ratio.
///
/// # Errors
/// Rejects invalid physical dimensions or camera parameters.
pub fn camera(extent: [u32; 2], c: &Camera) -> Result<molgfx::Camera> {
    let aspect = num_traits::ToPrimitive::to_f32(&extent[0])
        .ok_or_else(|| io::Error::other("invalid width"))?
        / num_traits::ToPrimitive::to_f32(&extent[1])
            .ok_or_else(|| io::Error::other("invalid height"))?;
    Ok(molgfx::camera::perspective(
        c.position,
        c.target,
        c.up,
        c.fov_y_degrees.to_radians(),
        aspect,
        c.near,
        c.far,
    )?)
}

/// Fits the authored selection using the gallery's shared camera convention.
///
/// # Errors
/// Rejects empty selections, invalid selections and invalid fit parameters.
pub fn fit_camera(scene: &Scene, fit: &FitCamera) -> Result<Camera> {
    if !fit.margin.is_finite()
        || fit.margin <= 0.0
        || !fit.fov_y_degrees.is_finite()
        || fit.fov_y_degrees <= 0.0
        || fit.fov_y_degrees >= 180.0
    {
        return Err(io::Error::other(
            "camera fit requires a positive margin and valid field of view",
        )
        .into());
    }
    let bounds = scene
        .selection_bounds(fit.selection.as_str())?
        .ok_or_else(|| io::Error::other("fit selection matches no atoms"))?;
    let sphere = bounds.bounding_sphere();
    let radius = sphere.radius * fit.margin;
    let distance = radius / (fit.fov_y_degrees.to_radians() * 0.5).sin();
    let target = sphere.center.to_array();
    Ok(Camera {
        position: [target[0], target[1], target[2] + distance],
        target,
        up: [0.0, 1.0, 0.0],
        fov_y_degrees: fit.fov_y_degrees,
        near: (distance - radius).max(0.1),
        far: distance + radius,
    })
}

/// Resolves the case's authored effects on its fixed native quality recipe.
///
/// # Errors
/// Rejects unknown effects and invalid effect parameters.
pub fn profile(fixture: &Fixture, output: MeasuredOutput) -> Result<profile::RenderProfile> {
    let profile = match output {
        MeasuredOutput::Converged => profile::converged(),
        MeasuredOutput::Interactive => profile::highest_fixed(120),
    };
    let effect = fixture.details.get("effect").and_then(Value::as_str);
    // These paired scenes need their molecular highlights above the optical
    // bright-pass threshold while keeping the fallback background below it.
    let profile = if matches!(effect, Some("bloom" | "bloom_control")) {
        profile
            .with_effect(profile::Effect::Lighting(profile::LightingEnvironment {
                key_strength: 4.0,
                specular_strength: 2.0,
                ..profile::LightingEnvironment::soft_key()
            }))?
            .with_effect(profile::Effect::Backdrop(profile::BackdropStyle {
                top: profile::Rgba8::opaque(22, 27, 36),
                bottom: profile::Rgba8::opaque(10, 14, 22),
                glow_strength: 0.0,
                ..profile::BackdropStyle::compositing()
            }))?
    } else {
        profile
    };
    let profile = match effect {
        Some("bloom") => profile.with_effect(profile::Effect::Bloom(profile::BloomStyle {
            threshold: 0.85,
            intensity: 0.8,
            radius: 4.0,
        }))?,
        Some("dof") => profile.with_effect(profile::Effect::DepthOfField(
            profile::DepthOfField::macro_lens(),
        ))?,
        Some("motion_blur") => profile.with_effect(profile::Effect::MotionBlur(
            profile::MotionBlur::restrained(),
        ))?,
        Some("depth_cue") => profile.with_effect(profile::Effect::DepthCue(
            profile::DepthCue::new(fixture.camera.near * 1.25, fixture.camera.far * 0.85, 0.65)?,
        ))?,
        Some("shape_cues") => profile.with_effect(profile::Effect::ShapeCues(
            profile::ShapeCueStyle::restrained(),
        ))?,
        Some("bloom_control") | None => profile,
        Some(other) => {
            return Err(io::Error::other(format!("unknown gallery effect: {other}")).into());
        }
    };
    Ok(profile)
}
