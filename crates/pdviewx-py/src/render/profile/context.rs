//! Backdrop, lighting, and illustration context.

use crate::math::{PyRgba8, PyVec3};
use pyo3::prelude::*;

#[pyclass(name = "BackdropStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyBackdropStyle(pub(crate) pdviewx::BackdropStyle);

#[pymethods]
impl PyBackdropStyle {
    #[new]
    #[pyo3(signature = (top=None, bottom=None, glow_color=None, glow_strength=0.08))]
    fn new(
        top: Option<PyRgba8>,
        bottom: Option<PyRgba8>,
        glow_color: Option<PyRgba8>,
        glow_strength: f32,
    ) -> Self {
        let default = pdviewx::BackdropStyle::default();
        Self(pdviewx::BackdropStyle {
            top: top.map_or(default.top, |value| value.0),
            bottom: bottom.map_or(default.bottom, |value| value.0),
            glow_color: glow_color.map_or(default.glow_color, |value| value.0),
            glow_strength,
        })
    }

    #[staticmethod]
    fn compositing() -> Self {
        Self(pdviewx::BackdropStyle::compositing())
    }

    #[staticmethod]
    fn transparent() -> Self {
        Self(pdviewx::BackdropStyle::transparent())
    }

    #[staticmethod]
    fn studio() -> Self {
        Self(pdviewx::BackdropStyle::studio())
    }

    #[getter]
    fn top(&self) -> PyRgba8 {
        self.0.top.into()
    }

    #[getter]
    fn bottom(&self) -> PyRgba8 {
        self.0.bottom.into()
    }

    #[getter]
    fn glow_color(&self) -> PyRgba8 {
        self.0.glow_color.into()
    }

    #[getter]
    fn glow_strength(&self) -> f32 {
        self.0.glow_strength
    }
}

#[pyclass(name = "LightingEnvironment", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyLightingEnvironment(pub(crate) pdviewx::LightingEnvironment);

#[pymethods]
impl PyLightingEnvironment {
    #[staticmethod]
    fn neutral() -> Self {
        Self(pdviewx::LightingEnvironment::neutral())
    }

    #[staticmethod]
    fn documentary() -> Self {
        Self(pdviewx::LightingEnvironment::documentary())
    }

    #[getter]
    fn zenith(&self) -> PyRgba8 {
        self.0.zenith.into()
    }

    #[getter]
    fn horizon(&self) -> PyRgba8 {
        self.0.horizon.into()
    }

    #[getter]
    fn ground(&self) -> PyRgba8 {
        self.0.ground.into()
    }

    #[getter]
    fn key_direction(&self) -> PyVec3 {
        PyVec3(self.0.key_direction)
    }

    #[getter]
    fn fill_direction(&self) -> PyVec3 {
        PyVec3(self.0.fill_direction)
    }
}

#[pyclass(name = "IllustrationStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyIllustrationStyle(pub(crate) pdviewx::IllustrationStyle);

#[pymethods]
impl PyIllustrationStyle {
    #[new]
    #[pyo3(signature = (silhouette_strength=0.0, cavity_strength=0.0, depth_cue_strength=0.0))]
    fn new(silhouette_strength: f32, cavity_strength: f32, depth_cue_strength: f32) -> Self {
        Self(pdviewx::IllustrationStyle {
            silhouette_strength,
            cavity_strength,
            depth_cue_strength,
        })
    }

    #[staticmethod]
    fn publication() -> Self {
        Self(pdviewx::IllustrationStyle::publication())
    }

    #[getter]
    fn silhouette_strength(&self) -> f32 {
        self.0.silhouette_strength
    }

    #[getter]
    fn cavity_strength(&self) -> f32 {
        self.0.cavity_strength
    }

    #[getter]
    fn depth_cue_strength(&self) -> f32 {
        self.0.depth_cue_strength
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyBackdropStyle>()?;
    module.add_class::<PyLightingEnvironment>()?;
    module.add_class::<PyIllustrationStyle>()
}
