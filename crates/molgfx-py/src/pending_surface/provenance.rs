//! Owned projection of borrowed scene provenance.

use crate::core::{PyEntityRef, PyScene};
use crate::memory::PyMemoryOwnership;
use pyo3::prelude::*;

#[pyclass(name = "ProvenanceDetail", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyProvenanceDetail {
    Atom,
    Bond,
    DynamicBond,
    Interaction,
    Guide,
    Annotation,
    Measurement,
    Primitive,
    Mesh,
    Unknown,
}

fn detail(value: molgfx::core::ProvenanceDetail<'_>) -> PyProvenanceDetail {
    match value {
        molgfx::core::ProvenanceDetail::Atom(_) => PyProvenanceDetail::Atom,
        molgfx::core::ProvenanceDetail::Bond(_) => PyProvenanceDetail::Bond,
        molgfx::core::ProvenanceDetail::DynamicBond(_) => PyProvenanceDetail::DynamicBond,
        molgfx::core::ProvenanceDetail::Interaction(_) => PyProvenanceDetail::Interaction,
        molgfx::core::ProvenanceDetail::Guide(_) => PyProvenanceDetail::Guide,
        molgfx::core::ProvenanceDetail::Annotation(_) => PyProvenanceDetail::Annotation,
        molgfx::core::ProvenanceDetail::Measurement(_) => PyProvenanceDetail::Measurement,
        molgfx::core::ProvenanceDetail::Primitive(_) => PyProvenanceDetail::Primitive,
        molgfx::core::ProvenanceDetail::Mesh(_) => PyProvenanceDetail::Mesh,
        _ => PyProvenanceDetail::Unknown,
    }
}

#[pyclass(name = "EntityProvenance", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyEntityProvenance {
    entity: PyEntityRef,
    entry_id: Option<String>,
    entry_title: Option<String>,
    method: Option<String>,
    resolution: Option<f32>,
    detail: PyProvenanceDetail,
}

impl From<molgfx::core::EntityProvenance<'_>> for PyEntityProvenance {
    fn from(value: molgfx::core::EntityProvenance<'_>) -> Self {
        Self {
            entity: value.entity.into(),
            entry_id: value.entry.id.as_deref().map(str::to_owned),
            entry_title: value.entry.title.as_deref().map(str::to_owned),
            method: value.entry.method.as_deref().map(str::to_owned),
            resolution: value.entry.resolution,
            detail: detail(value.detail),
        }
    }
}

#[pymethods]
impl PyEntityProvenance {
    #[staticmethod]
    fn copy_from_scene(scene: PyRef<'_, PyScene>, entity: PyEntityRef) -> Option<Self> {
        scene.inner.provenance(entity.0).map(Into::into)
    }

    #[getter]
    fn entity(&self) -> PyEntityRef {
        self.entity
    }

    #[getter]
    fn entry_id(&self) -> Option<String> {
        self.entry_id.clone()
    }

    #[getter]
    fn entry_title(&self) -> Option<String> {
        self.entry_title.clone()
    }

    #[getter]
    fn method(&self) -> Option<String> {
        self.method.clone()
    }

    #[getter]
    fn resolution(&self) -> Option<f32> {
        self.resolution
    }

    #[getter]
    fn detail(&self) -> PyProvenanceDetail {
        self.detail
    }

    #[getter]
    fn ownership(&self) -> PyMemoryOwnership {
        PyMemoryOwnership::Copied
    }
}
