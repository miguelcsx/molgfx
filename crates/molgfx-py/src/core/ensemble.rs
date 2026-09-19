//! Python adapter for weighted structural ensembles.

use crate::core::PyStructureHandle;
use crate::error::core;
use pyo3::prelude::*;

#[pyclass(name = "Ensemble", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyEnsemble(pub(crate) molgfx::core::Ensemble);

#[pymethods]
impl PyEnsemble {
    /// Validates and normalizes positive caller populations into weights
    /// summing to one.
    #[new]
    fn new(
        members: Vec<PyStructureHandle>,
        weights: Vec<f32>,
        provenance: String,
    ) -> PyResult<Self> {
        let members = members
            .into_iter()
            .map(|member| member.0)
            .collect::<std::sync::Arc<[_]>>();
        core(molgfx::core::Ensemble::new(members, &weights, provenance)).map(Self)
    }

    /// Member placements in deterministic caller order.
    #[getter]
    fn members(&self) -> Vec<PyStructureHandle> {
        self.0.members().iter().copied().map(Into::into).collect()
    }

    /// Normalized probabilities summing to one within float error.
    #[getter]
    fn weights(&self) -> Vec<f32> {
        self.0.weights().to_vec()
    }

    /// Caller computation or dataset identifier.
    #[getter]
    fn provenance(&self) -> String {
        self.0.provenance().to_owned()
    }

    /// Stable index of the highest-weight member; earliest wins ties.
    #[getter]
    fn dominant_index(&self) -> usize {
        self.0.dominant_index()
    }

    fn __repr__(&self) -> String {
        format!(
            "Ensemble(members={}, provenance={:?})",
            self.0.members().len(),
            self.0.provenance()
        )
    }
}

impl From<molgfx::core::Ensemble> for PyEnsemble {
    fn from(value: molgfx::core::Ensemble) -> Self {
        Self(value)
    }
}
