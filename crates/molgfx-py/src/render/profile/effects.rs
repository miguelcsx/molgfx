//! Presentation effects and composable effect layers.

use super::{
    PyBackdropStyle, PyDisplayTransform, PyFocusTarget, PyIllustrationStyle, PyLightingEnvironment,
};
use pyo3::prelude::*;

#[pyclass(name = "DepthOfField", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyDepthOfField(pub(crate) molgfx::render::DepthOfField);

#[pymethods]
impl PyDepthOfField {
    #[new]
    #[pyo3(signature = (focal_length_mm=50.0, f_number=8.0, sensor_width_mm=36.0, max_blur_pixels=14.0, blade_count=7, focus=None))]
    fn new(
        focal_length_mm: f32,
        f_number: f32,
        sensor_width_mm: f32,
        max_blur_pixels: f32,
        blade_count: u8,
        focus: Option<PyFocusTarget>,
    ) -> Self {
        let default = molgfx::render::DepthOfField::cinematic();
        Self(molgfx::render::DepthOfField {
            focal_length_mm,
            f_number,
            sensor_width_mm,
            max_blur_pixels,
            blade_count,
            focus: focus.map_or(default.focus, |value| value.0),
        })
    }

    #[staticmethod]
    fn cinematic() -> Self {
        Self(molgfx::render::DepthOfField::cinematic())
    }

    #[getter]
    fn focal_length_mm(&self) -> f32 {
        self.0.focal_length_mm
    }

    #[getter]
    fn f_number(&self) -> f32 {
        self.0.f_number
    }

    #[getter]
    fn sensor_width_mm(&self) -> f32 {
        self.0.sensor_width_mm
    }

    #[getter]
    fn max_blur_pixels(&self) -> f32 {
        self.0.max_blur_pixels
    }

    #[getter]
    fn blade_count(&self) -> u8 {
        self.0.blade_count
    }

    #[getter]
    fn focus(&self) -> PyFocusTarget {
        PyFocusTarget(self.0.focus)
    }
}

#[pyclass(name = "MotionBlur", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyMotionBlur(pub(crate) molgfx::render::MotionBlur);

#[pymethods]
impl PyMotionBlur {
    #[new]
    #[pyo3(signature = (shutter=0.55, max_blur_pixels=18.0))]
    fn new(shutter: f32, max_blur_pixels: f32) -> Self {
        Self(molgfx::render::MotionBlur {
            shutter,
            max_blur_pixels,
        })
    }

    #[staticmethod]
    fn cinematic() -> Self {
        Self(molgfx::render::MotionBlur::cinematic())
    }

    #[getter]
    fn shutter(&self) -> f32 {
        self.0.shutter
    }

    #[getter]
    fn max_blur_pixels(&self) -> f32 {
        self.0.max_blur_pixels
    }
}

#[pyclass(name = "BloomStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyBloomStyle(pub(crate) molgfx::render::BloomStyle);

#[pymethods]
impl PyBloomStyle {
    #[new]
    #[pyo3(signature = (threshold=1.7, intensity=0.26, radius=2.0))]
    fn new(threshold: f32, intensity: f32, radius: f32) -> Self {
        Self(molgfx::render::BloomStyle {
            threshold,
            intensity,
            radius,
        })
    }

    #[staticmethod]
    fn cinematic() -> Self {
        Self(molgfx::render::BloomStyle::cinematic())
    }

    #[getter]
    fn threshold(&self) -> f32 {
        self.0.threshold
    }

    #[getter]
    fn intensity(&self) -> f32 {
        self.0.intensity
    }

    #[getter]
    fn radius(&self) -> f32 {
        self.0.radius
    }
}

#[pyclass(name = "PresentationEffect", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyPresentationEffect(pub(crate) molgfx::render::PresentationEffect);

#[pymethods]
impl PyPresentationEffect {
    #[staticmethod]
    fn illustration(value: PyIllustrationStyle) -> Self {
        Self(molgfx::render::PresentationEffect::Illustration(value.0))
    }

    #[staticmethod]
    fn depth_of_field(value: PyDepthOfField) -> Self {
        Self(molgfx::render::PresentationEffect::DepthOfField(value.0))
    }

    #[staticmethod]
    fn motion_blur(value: PyMotionBlur) -> Self {
        Self(molgfx::render::PresentationEffect::MotionBlur(value.0))
    }

    #[staticmethod]
    fn backdrop(value: PyBackdropStyle) -> Self {
        Self(molgfx::render::PresentationEffect::Backdrop(value.0))
    }

    #[staticmethod]
    fn lighting(value: PyLightingEnvironment) -> Self {
        Self(molgfx::render::PresentationEffect::Lighting(value.0))
    }

    #[staticmethod]
    fn display(value: PyDisplayTransform) -> Self {
        Self(molgfx::render::PresentationEffect::Display(value.0))
    }

    #[staticmethod]
    fn bloom(value: PyBloomStyle) -> Self {
        Self(molgfx::render::PresentationEffect::Bloom(value.0))
    }
}

#[pyclass(name = "EffectLayer", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyEffectLayer(pub(crate) molgfx::render::EffectLayer);

#[pymethods]
impl PyEffectLayer {
    #[new]
    fn new(effect: PyPresentationEffect) -> Self {
        Self(molgfx::render::EffectLayer::new(effect.0))
    }

    fn with_weight(&self, value: f32) -> Self {
        Self(self.0.with_weight(value))
    }

    fn with_priority(&self, value: i16) -> Self {
        Self(self.0.with_priority(value))
    }
}
