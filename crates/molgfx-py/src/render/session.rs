//! Python adapter for reproducible render-session persistence.

use super::PyRenderProfile;
use crate::core::PyScene;
use crate::error::render;
use crate::math::PyCamera;
use pyo3::prelude::*;

#[pyclass(name = "RenderSession", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyRenderSession(pub(crate) molgfx::render::RenderSession);

#[pymethods]
impl PyRenderSession {
    #[staticmethod]
    fn capture(scene: &PyScene, camera: PyCamera, profile: PyRenderProfile) -> Self {
        Self(molgfx::render::RenderSession::new(
            &scene.inner,
            camera.inner,
            profile.0,
        ))
    }

    #[staticmethod]
    fn from_json(source: &str) -> PyResult<Self> {
        render(molgfx::render::RenderSession::from_json(source)).map(Self)
    }

    fn to_json(&self) -> PyResult<String> {
        render(self.0.to_json())
    }

    #[getter]
    fn schema(&self) -> u16 {
        self.0.schema
    }

    #[getter]
    fn camera(&self) -> PyCamera {
        PyCamera {
            inner: self.0.camera,
        }
    }

    #[getter]
    fn profile(&self) -> PyRenderProfile {
        PyRenderProfile(self.0.profile.clone())
    }
}
