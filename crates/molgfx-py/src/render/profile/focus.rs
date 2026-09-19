//! Render focus target conversion.

use crate::core::PySelectionHandle;
use crate::math::PyVec3;
use pyo3::prelude::*;

#[pyclass(name = "FocusTarget", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyFocusTarget(pub(crate) molgfx::render::FocusTarget);

#[pymethods]
impl PyFocusTarget {
    #[staticmethod]
    fn camera_target() -> Self {
        Self(molgfx::render::FocusTarget::CameraTarget)
    }

    #[staticmethod]
    fn distance(value: f32) -> Self {
        Self(molgfx::render::FocusTarget::Distance(value))
    }

    #[staticmethod]
    fn world_point(value: PyVec3) -> Self {
        Self(molgfx::render::FocusTarget::WorldPoint(value.0))
    }

    #[staticmethod]
    fn selection(value: PySelectionHandle) -> Self {
        Self(molgfx::render::FocusTarget::Selection(value.0))
    }

    #[getter]
    fn kind(&self) -> &'static str {
        match self.0 {
            molgfx::render::FocusTarget::CameraTarget => "camera_target",
            molgfx::render::FocusTarget::Distance(_) => "distance",
            molgfx::render::FocusTarget::WorldPoint(_) => "world_point",
            molgfx::render::FocusTarget::Selection(_) => "selection",
        }
    }
}
