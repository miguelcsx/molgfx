//! Opaque Python semantic identifiers.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

macro_rules! semantic_id {
    ($rust:ident, $python:literal) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[pyclass(name = $python, frozen, skip_from_py_object)]
        pub(super) struct $rust(pub(super) u64);

        #[pymethods]
        impl $rust {
            #[getter]
            const fn value(&self) -> u64 {
                self.0
            }

            const fn __int__(&self) -> u64 {
                self.0
            }

            const fn __index__(&self) -> u64 {
                self.0
            }

            fn __repr__(&self) -> String {
                format!("{}({})", $python, self.0)
            }
        }
    };
}

semantic_id!(PyRepresentationId, "RepresentationId");
semantic_id!(PyStructureId, "StructureId");
semantic_id!(PyVolumeId, "VolumeId");
semantic_id!(PyAnnotationId, "AnnotationId");
semantic_id!(PyMeasurementId, "MeasurementId");
semantic_id!(PyScientificInteractionId, "ScientificInteractionId");
semantic_id!(PyTrajectoryId, "TrajectoryId");

pub(super) enum PySceneId {
    Representation(u64),
    Volume(u64),
    Annotation(u64),
    Measurement(u64),
    ScientificInteraction(u64),
    Trajectory(u64),
}

pub(super) fn representation_id(value: &Bound<'_, PyAny>) -> PyResult<u64> {
    Ok(value
        .extract::<PyRef<'_, PyRepresentationId>>()
        .map(|id| id.0)?)
}

pub(super) fn structure_id(value: &Bound<'_, PyAny>) -> PyResult<u64> {
    Ok(value.extract::<PyRef<'_, PyStructureId>>().map(|id| id.0)?)
}

impl PySceneId {
    pub(super) fn into_python(self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self {
            Self::Representation(value) => Ok(Py::new(py, PyRepresentationId(value))?.into_any()),
            Self::Volume(value) => Ok(Py::new(py, PyVolumeId(value))?.into_any()),
            Self::Annotation(value) => Ok(Py::new(py, PyAnnotationId(value))?.into_any()),
            Self::Measurement(value) => Ok(Py::new(py, PyMeasurementId(value))?.into_any()),
            Self::ScientificInteraction(value) => {
                Ok(Py::new(py, PyScientificInteractionId(value))?.into_any())
            }
            Self::Trajectory(value) => Ok(Py::new(py, PyTrajectoryId(value))?.into_any()),
        }
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRepresentationId>()?;
    module.add_class::<PyStructureId>()?;
    module.add_class::<PyVolumeId>()?;
    module.add_class::<PyAnnotationId>()?;
    module.add_class::<PyMeasurementId>()?;
    module.add_class::<PyScientificInteractionId>()?;
    module.add_class::<PyTrajectoryId>()
}
