//! Python enums and exact row-domain identities.

use super::super::{
    PyInstanceBatchHandle, PyPointBatchHandle, PyRelationBatchHandle, PyStructureHandle,
};
use pyo3::prelude::*;

#[pyclass(name = "RelationPattern", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyRelationPattern {
    Solid,
    Dashed,
    Dotted,
    Spring,
}

impl From<PyRelationPattern> for pdviewx::RelationPattern {
    fn from(value: PyRelationPattern) -> Self {
        match value {
            PyRelationPattern::Solid => Self::Solid,
            PyRelationPattern::Dashed => Self::Dashed,
            PyRelationPattern::Dotted => Self::Dotted,
            PyRelationPattern::Spring => Self::Spring,
        }
    }
}

#[pyclass(name = "RowDomain", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyRowDomain(pub(crate) pdviewx::RowDomain);

#[pymethods]
impl PyRowDomain {
    #[staticmethod]
    fn atoms(handle: PyStructureHandle) -> Self {
        Self(pdviewx::RowDomain::Atoms(handle.0))
    }

    #[staticmethod]
    fn points(handle: PyPointBatchHandle) -> Self {
        Self(pdviewx::RowDomain::Points(handle.0))
    }

    #[staticmethod]
    fn instances(handle: PyInstanceBatchHandle) -> Self {
        Self(pdviewx::RowDomain::Instances(handle.0))
    }

    #[staticmethod]
    fn template_parts(handle: PyInstanceBatchHandle) -> Self {
        Self(pdviewx::RowDomain::TemplateParts(handle.0))
    }

    #[staticmethod]
    fn relations(handle: PyRelationBatchHandle) -> Self {
        Self(pdviewx::RowDomain::Relations(handle.0))
    }
}
