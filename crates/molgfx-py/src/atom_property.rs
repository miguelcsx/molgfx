//! Ownership-explicit Python adapters for per-atom scalar properties.

use crate::core::PyStructureHandle;
use crate::error::core;
use crate::memory::PyMemoryOwnership;
use crate::values::PyScalarFieldSemantics;
use numpy::PyReadonlyArray1;
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

impl From<PyAtomPropertyMeaning> for molgfx::AtomPropertyMeaning {
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
pub(crate) struct PyAtomProperty(pub(crate) molgfx::AtomProperty);

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
        core(molgfx::AtomProperty::new(
            owner.0,
            name,
            Arc::from(values),
            meaning.into(),
            semantics.0,
        ))
        .map(Self)
    }

    fn copy_values(&self) -> Vec<f32> {
        self.0.values().to_vec()
    }

    #[getter]
    fn numpy_ownership(&self) -> PyMemoryOwnership {
        PyMemoryOwnership::Copied
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyAtomPropertyMeaning>()?;
    module.add_class::<PyAtomProperty>()
}
