//! Python constructors for the same validated scene presentation effects.

use molgfx::profile::{self, Effect};
use pyo3::prelude::*;
use pyo3::types::PyModule;

#[derive(Clone, Debug)]
#[pyclass(name = "Effect", frozen, skip_from_py_object)]
pub(super) struct PyEffect(pub(super) Effect);

#[pymethods]
impl PyEffect {
    #[getter]
    fn kind(&self) -> &'static str {
        match self.0.kind() {
            profile::EffectKind::DepthCue => "depth_cue",
            profile::EffectKind::AntiAliasing => "anti_aliasing",
            profile::EffectKind::Bloom => "bloom",
            profile::EffectKind::DepthOfField => "depth_of_field",
            profile::EffectKind::MotionBlur => "motion_blur",
            profile::EffectKind::Backdrop => "backdrop",
            profile::EffectKind::Lighting => "lighting",
            profile::EffectKind::ShapeCues => "shape_cues",
            profile::EffectKind::Display => "display",
        }
    }
}

fn checked(effect: Effect) -> PyResult<PyEffect> {
    effect
        .validate()
        .map(PyEffect)
        .map_err(crate::binding::error)
}

#[pyfunction]
#[pyo3(signature = (*, near_distance, far_distance, strength))]
fn depth_cue(near_distance: f32, far_distance: f32, strength: f32) -> PyResult<PyEffect> {
    let cue = profile::DepthCue::new(near_distance, far_distance, strength)
        .map_err(crate::binding::error)?;
    checked(Effect::DepthCue(cue))
}

#[pyfunction]
fn fxaa() -> PyEffect {
    PyEffect(Effect::AntiAliasing(profile::AntiAliasing::Fxaa))
}

#[pyfunction]
fn no_anti_aliasing() -> PyEffect {
    PyEffect(Effect::AntiAliasing(profile::AntiAliasing::Off))
}

#[pyfunction]
#[pyo3(signature = (*, threshold=1.7, intensity=0.26, radius=2.0))]
fn bloom(threshold: f32, intensity: f32, radius: f32) -> PyResult<PyEffect> {
    checked(Effect::Bloom(profile::BloomStyle {
        threshold,
        intensity,
        radius,
    }))
}

#[pyfunction]
#[pyo3(signature = (*, focal_length_mm=50.0, f_number=8.0, sensor_width_mm=36.0, max_blur_pixels=14.0, blade_count=7, focus_distance=None))]
fn depth_of_field(
    focal_length_mm: f32,
    f_number: f32,
    sensor_width_mm: f32,
    max_blur_pixels: f32,
    blade_count: u8,
    focus_distance: Option<f32>,
) -> PyResult<PyEffect> {
    let focus = match focus_distance {
        Some(distance) => profile::FocusTarget::Distance(distance),
        None => profile::FocusTarget::CameraTarget,
    };
    checked(Effect::DepthOfField(profile::DepthOfField {
        focal_length_mm,
        f_number,
        sensor_width_mm,
        max_blur_pixels,
        blade_count,
        focus,
    }))
}

#[pyfunction]
#[pyo3(signature = (*, shutter=0.55, max_blur_pixels=18.0))]
fn motion_blur(shutter: f32, max_blur_pixels: f32) -> PyResult<PyEffect> {
    checked(Effect::MotionBlur(profile::MotionBlur {
        shutter,
        max_blur_pixels,
    }))
}

#[pyfunction]
#[pyo3(signature = (*, top=(238, 241, 245), bottom=(206, 214, 223), glow_color=(255, 255, 255), glow_strength=0.3))]
fn backdrop_gradient(
    top: (u8, u8, u8),
    bottom: (u8, u8, u8),
    glow_color: (u8, u8, u8),
    glow_strength: f32,
) -> PyResult<PyEffect> {
    checked(Effect::Backdrop(profile::BackdropStyle {
        top: profile::Rgba8::opaque(top.0, top.1, top.2),
        bottom: profile::Rgba8::opaque(bottom.0, bottom.1, bottom.2),
        glow_color: profile::Rgba8::opaque(glow_color.0, glow_color.1, glow_color.2),
        glow_strength,
    }))
}

#[pyfunction]
#[pyo3(signature = (*, key_strength=1.35, fill_strength=0.30, shadow_strength=0.66))]
fn lighting(key_strength: f32, fill_strength: f32, shadow_strength: f32) -> PyResult<PyEffect> {
    checked(Effect::Lighting(profile::LightingEnvironment {
        key_strength,
        fill_strength,
        shadow_strength,
        ..profile::LightingEnvironment::soft_key()
    }))
}

#[pyfunction]
#[pyo3(signature = (*, silhouette_strength=0.65, cavity_strength=0.35, depth_cue_strength=0.15, posterize_levels=0.0, motion_persistence=0.0, outline_width=0.0))]
fn shape_cues(
    silhouette_strength: f32,
    cavity_strength: f32,
    depth_cue_strength: f32,
    posterize_levels: f32,
    motion_persistence: f32,
    outline_width: f32,
) -> PyResult<PyEffect> {
    checked(Effect::ShapeCues(profile::ShapeCueStyle {
        silhouette_strength,
        cavity_strength,
        depth_cue_strength,
        posterize_levels,
        motion_persistence,
        outline_width,
    }))
}

#[pyfunction]
#[pyo3(signature = (*, exposure_ev=0.22, contrast=1.18, saturation=1.3, vignette_strength=0.16, peak_luminance_nits=100.0))]
fn display(
    exposure_ev: f32,
    contrast: f32,
    saturation: f32,
    vignette_strength: f32,
    peak_luminance_nits: f32,
) -> PyResult<PyEffect> {
    checked(Effect::Display(profile::DisplayTransform {
        exposure_ev,
        contrast,
        saturation,
        vignette_strength,
        peak_luminance_nits,
        ..profile::DisplayTransform::filmic()
    }))
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyEffect>()?;
    let effect = PyModule::new(module.py(), "effect")?;
    effect.add_function(wrap_pyfunction!(depth_cue, &effect)?)?;
    effect.add_function(wrap_pyfunction!(fxaa, &effect)?)?;
    effect.add_function(wrap_pyfunction!(no_anti_aliasing, &effect)?)?;
    effect.add_function(wrap_pyfunction!(bloom, &effect)?)?;
    effect.add_function(wrap_pyfunction!(depth_of_field, &effect)?)?;
    effect.add_function(wrap_pyfunction!(motion_blur, &effect)?)?;
    effect.add_function(wrap_pyfunction!(backdrop_gradient, &effect)?)?;
    effect.add_function(wrap_pyfunction!(lighting, &effect)?)?;
    effect.add_function(wrap_pyfunction!(shape_cues, &effect)?)?;
    effect.add_function(wrap_pyfunction!(display, &effect)?)?;
    module.add_submodule(&effect)
}
