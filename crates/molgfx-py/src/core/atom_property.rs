//! Ownership-explicit Python adapters for per-atom scalar properties.

use crate::core::PyStructureHandle;
use crate::error::core;
use crate::memory::PyMemoryOwnership;
use crate::values::PyScalarFieldSemantics;
use numpy::{PyArray1, PyReadonlyArray1};
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "AtomPropertyMeaning", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyAtomPropertyMeaning {
    Generic,
    Confidence,
    Occupancy,
    LocalResolution,
    Flexibility,
    Charge,
    Hydrophobicity,
    Exposure,
}

impl From<PyAtomPropertyMeaning> for molgfx::core::AtomPropertyMeaning {
    fn from(value: PyAtomPropertyMeaning) -> Self {
        match value {
            PyAtomPropertyMeaning::Generic => Self::Generic,
            PyAtomPropertyMeaning::Confidence => Self::Confidence,
            PyAtomPropertyMeaning::Occupancy => Self::Occupancy,
            PyAtomPropertyMeaning::LocalResolution => Self::LocalResolution,
            PyAtomPropertyMeaning::Flexibility => Self::Flexibility,
            PyAtomPropertyMeaning::Charge => Self::Charge,
            PyAtomPropertyMeaning::Hydrophobicity => Self::Hydrophobicity,
            PyAtomPropertyMeaning::Exposure => Self::Exposure,
        }
    }
}

#[pyclass(name = "AtomProperty", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyAtomProperty(pub(crate) molgfx::core::AtomProperty);

#[pymethods]
impl PyAtomProperty {
    /// Copies one explicit C-contiguous float32 row into Rust-owned storage.
    #[staticmethod]
    fn copy_from_numpy(
        owner: PyStructureHandle,
        name: &str,
        values: PyReadonlyArray1<'_, f32>,
        meaning: PyAtomPropertyMeaning,
        semantics: PyScalarFieldSemantics,
    ) -> PyResult<Self> {
        let values = values
            .as_slice()
            .map_err(|_| crate::error::value("values must be C-contiguous float32"))?;
        core(molgfx::core::AtomProperty::new(
            owner.0,
            name,
            Arc::from(values),
            meaning.into(),
            semantics.0,
        ))
        .map(Self)
    }

    fn copy_values<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f32>> {
        PyArray1::from_slice(py, self.0.values())
    }

    #[getter]
    fn numpy_ownership(&self) -> PyMemoryOwnership {
        PyMemoryOwnership::Copied
    }
}
