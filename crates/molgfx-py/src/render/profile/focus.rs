//! Render focus target conversion.

use crate::core::PySelectionHandle;
use crate::math::PyVec3;
use pyo3::prelude::*;

#[pyclass(name = "FocusTarget", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyFocusTarget(pub(crate) molgfx::FocusTarget);

#[pymethods]
impl PyFocusTarget {
    #[staticmethod]
    fn camera_target() -> Self {
        Self(molgfx::FocusTarget::CameraTarget)
    }

    #[staticmethod]
    fn distance(value: f32) -> Self {
        Self(molgfx::FocusTarget::Distance(value))
    }

    #[staticmethod]
    fn world_point(value: PyVec3) -> Self {
        Self(molgfx::FocusTarget::WorldPoint(value.0))
    }

    #[staticmethod]
    fn selection(value: PySelectionHandle) -> Self {
        Self(molgfx::FocusTarget::Selection(value.0))
    }

    #[getter]
    fn kind(&self) -> &'static str {
        match self.0 {
            molgfx::FocusTarget::CameraTarget => "camera_target",
            molgfx::FocusTarget::Distance(_) => "distance",
            molgfx::FocusTarget::WorldPoint(_) => "world_point",
            molgfx::FocusTarget::Selection(_) => "selection",
        }
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFocusTarget>()
}
