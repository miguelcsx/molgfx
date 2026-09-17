//! Caller-computed validation markers exposed without detector logic.

use crate::annotations::{PyAnnotationAnchor, PyMarkerStyle};
use crate::core::{PyAnnotationHandle, PyScene, PyStructureHandle};
use crate::error::core;
use pyo3::prelude::*;

#[pyclass(name = "ValidationKind", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyValidationKind {
    Clash,
    Geometry,
    Density,
    Other,
}

impl From<PyValidationKind> for molgfx::ValidationKind {
    fn from(value: PyValidationKind) -> Self {
        match value {
            PyValidationKind::Clash => Self::Clash,
            PyValidationKind::Geometry => Self::Geometry,
            PyValidationKind::Density => Self::Density,
            PyValidationKind::Other => Self::Other,
        }
    }
}

#[pyclass(name = "ValidationMarker", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyValidationMarker(molgfx::ValidationMarker);

#[pymethods]
impl PyValidationMarker {
    #[new]
    fn new(
        owner: PyStructureHandle,
        anchor: PyAnnotationAnchor,
        kind: PyValidationKind,
        severity: f32,
        style: PyMarkerStyle,
    ) -> PyResult<Self> {
        core(molgfx::ValidationMarker::new(
            owner.0,
            anchor.0,
            kind.into(),
            severity,
            style.0,
        ))
        .map(Self)
    }

    #[getter]
    fn severity(&self) -> f32 {
        self.0.severity
    }
}

#[pymethods]
impl PyScene {
    fn add_validation_marker(
        &mut self,
        marker: PyValidationMarker,
    ) -> PyResult<PyAnnotationHandle> {
        core(self.inner.add_validation_marker(marker.0)).map(Into::into)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyValidationKind>()?;
    module.add_class::<PyValidationMarker>()
}
