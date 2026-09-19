//! Render profile composition and resolved presentation plans.

use super::{
    PyBackdropStyle, PyBloomStyle, PyDepthOfField, PyDisplayTransform, PyEffectLayer,
    PyIllustrationStyle, PyLightingEnvironment, PyMotionBlur, PyPresentationEffect,
};
use pyo3::prelude::*;

#[pyclass(name = "RenderProfile", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyRenderProfile(pub(crate) molgfx::render::RenderProfile);

#[pymethods]
impl PyRenderProfile {
    #[staticmethod]
    fn inspection() -> Self {
        Self(molgfx::render::RenderProfile::inspection())
    }

    #[staticmethod]
    fn illustrative() -> Self {
        Self(molgfx::render::RenderProfile::illustrative())
    }

    #[staticmethod]
    fn cinematic() -> Self {
        Self(molgfx::render::RenderProfile::cinematic())
    }

    fn with_effect(&self, value: PyPresentationEffect) -> Self {
        Self(self.0.clone().with_effect(value.0))
    }

    fn with_layer(&self, value: PyEffectLayer) -> Self {
        Self(self.0.clone().with_layer(value.0))
    }

    #[getter]
    fn layer_count(&self) -> usize {
        self.0.layers().len()
    }
}

#[pyclass(name = "ResolvedRenderPlan", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyResolvedRenderPlan(pub(crate) molgfx::render::ResolvedRenderPlan);

#[pymethods]
impl PyResolvedRenderPlan {
    #[getter]
    fn illustration(&self) -> PyIllustrationStyle {
        PyIllustrationStyle(self.0.illustration())
    }

    #[getter]
    fn depth_of_field(&self) -> Option<PyDepthOfField> {
        self.0.depth_of_field().map(PyDepthOfField)
    }

    #[getter]
    fn motion_blur(&self) -> Option<PyMotionBlur> {
        self.0.motion_blur().map(PyMotionBlur)
    }

    #[getter]
    fn backdrop(&self) -> PyBackdropStyle {
        PyBackdropStyle(self.0.backdrop())
    }

    #[getter]
    fn lighting(&self) -> PyLightingEnvironment {
        PyLightingEnvironment(self.0.lighting())
    }

    #[getter]
    fn display(&self) -> PyDisplayTransform {
        PyDisplayTransform(self.0.display())
    }

    #[getter]
    fn bloom(&self) -> Option<PyBloomStyle> {
        self.0.bloom().map(PyBloomStyle)
    }
}
