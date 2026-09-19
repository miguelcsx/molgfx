//! Owned Python projection of cold scene provenance.

use crate::core::{PyEntityRef, PyScene};
use pyo3::prelude::*;

#[pyclass(name = "Provenance", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyProvenance {
    entity: PyEntityRef,
    entry_id: Option<String>,
    entry_title: Option<String>,
    method: Option<String>,
    resolution: Option<f32>,
    detail_kind: &'static str,
}

impl From<molgfx::core::EntityProvenance<'_>> for PyProvenance {
    fn from(value: molgfx::core::EntityProvenance<'_>) -> Self {
        let detail_kind = match value.detail {
            molgfx::core::ProvenanceDetail::Atom(_) => "atom",
            molgfx::core::ProvenanceDetail::Bond(_) => "bond",
            molgfx::core::ProvenanceDetail::DynamicBond(_) => "dynamic_bond",
            molgfx::core::ProvenanceDetail::Interaction(_) => "interaction",
            molgfx::core::ProvenanceDetail::Guide(_) => "guide",
            molgfx::core::ProvenanceDetail::Annotation(_) => "annotation",
            molgfx::core::ProvenanceDetail::Measurement(_) => "measurement",
            molgfx::core::ProvenanceDetail::Primitive(_) => "primitive",
            molgfx::core::ProvenanceDetail::Mesh(_) => "mesh",
            _ => "unknown",
        };
        Self {
            entity: value.entity.into(),
            entry_id: value.entry.id.as_deref().map(str::to_owned),
            entry_title: value.entry.title.as_deref().map(str::to_owned),
            method: value.entry.method.as_deref().map(str::to_owned),
            resolution: value.entry.resolution,
            detail_kind,
        }
    }
}

#[pymethods]
impl PyProvenance {
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
    fn detail_kind(&self) -> &'static str {
        self.detail_kind
    }
}

#[pymethods]
impl PyScene {
    fn provenance(&self, entity: PyEntityRef) -> Option<PyProvenance> {
        self.inner.provenance(entity.0).map(Into::into)
    }
}
