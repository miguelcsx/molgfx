//! Python result metadata for generic compositions.

use crate::core::PyRowDomain;
use pyo3::prelude::*;

#[pyclass(name = "GenericCompositionView", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyGenericCompositionView {
    domains: Vec<PyRowDomain>,
    normalized_weights: Vec<f32>,
}

impl From<molgfx::semantic::GenericCompositionView> for PyGenericCompositionView {
    fn from(value: molgfx::semantic::GenericCompositionView) -> Self {
        Self {
            domains: value.domains.into_iter().map(PyRowDomain).collect(),
            normalized_weights: value.normalized_weights,
        }
    }
}

#[pymethods]
impl PyGenericCompositionView {
    #[getter]
    fn domains(&self) -> Vec<PyRowDomain> {
        self.domains.clone()
    }

    #[getter]
    fn normalized_weights(&self) -> Vec<f32> {
        self.normalized_weights.clone()
    }
}
