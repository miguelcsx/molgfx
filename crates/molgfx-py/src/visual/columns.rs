//! Visual column identities, the slots a program reads and the descriptor
//! that attaches one.

use super::{
    PyColorExpr, PyScalarExpr, PyVectorExpr, PyVisualProgram, PyVisualProgramBuilder, PyVisualStyle,
};
use crate::core::generic_batches::attributes::PyAttributeKind;
use crate::core::{PyAtomPropertyHandle, PyAttributeHandle, PyRowDomain, PyScene};
use crate::error::visual;
use pyo3::prelude::*;

/// Scene-independent identity for one caller-owned paged visual column.
#[pyclass(name = "VisualColumnKey", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PyVisualColumnKey(pub(crate) molgfx::core::VisualColumnKey);

#[pymethods]
impl PyVisualColumnKey {
    #[new]
    fn new(value: u64) -> Self {
        Self(molgfx::core::VisualColumnKey(value))
    }

    #[getter]
    fn value(&self) -> u64 {
        self.0.0
    }

    fn __int__(&self) -> u64 {
        self.0.0
    }

    fn __repr__(&self) -> String {
        format!("VisualColumnKey({})", self.0.0)
    }
}

/// One typed column slot a visual program reads.
#[pyclass(name = "VisualAttributeRef", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PyVisualAttributeRef(pub(crate) molgfx::core::VisualAttributeRef);

impl From<molgfx::core::VisualAttributeRef> for PyVisualAttributeRef {
    fn from(value: molgfx::core::VisualAttributeRef) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyVisualAttributeRef {
    /// Slot bound to a scene attribute handle.
    #[staticmethod]
    fn attribute(handle: PyAttributeHandle, kind: PyAttributeKind) -> Self {
        Self(molgfx::core::VisualAttributeRef::Attribute {
            handle: handle.0,
            kind: kind.into(),
        })
    }

    /// Slot bound to a scene-independent caller-owned column.
    #[staticmethod]
    fn column(key: PyVisualColumnKey, kind: PyAttributeKind) -> Self {
        Self(molgfx::core::VisualAttributeRef::Column {
            key: key.0,
            kind: kind.into(),
        })
    }

    /// Slot bound to a pre-schema scalar property, retained for migration.
    #[staticmethod]
    fn legacy_scalar(property: PyAtomPropertyHandle) -> Self {
        Self(molgfx::core::VisualAttributeRef::LegacyScalar(property.0))
    }

    /// Physical layout the slot expects.
    #[getter]
    fn kind(&self) -> PyAttributeKind {
        self.0.kind().into()
    }

    /// Handle of a scene-bound slot, absent for the other two forms.
    #[getter]
    fn handle(&self) -> Option<PyAttributeHandle> {
        self.0.attribute().map(PyAttributeHandle)
    }

    /// Key of a scene-independent slot, absent for the other two forms.
    #[getter]
    fn key(&self) -> Option<PyVisualColumnKey> {
        self.0.column().map(PyVisualColumnKey)
    }

    /// Property of a migrating slot, absent for the other two forms.
    #[getter]
    fn property(&self) -> Option<PyAtomPropertyHandle> {
        self.0.legacy_scalar().map(PyAtomPropertyHandle)
    }

    fn __repr__(&self) -> String {
        match self.0 {
            molgfx::core::VisualAttributeRef::Attribute { handle, kind } => format!(
                "VisualAttributeRef.attribute(AttributeHandle({}, {}), {kind:?})",
                handle.row(),
                handle.generation()
            ),
            molgfx::core::VisualAttributeRef::Column { key, kind } => {
                format!(
                    "VisualAttributeRef.column(VisualColumnKey({}), {kind:?})",
                    key.0
                )
            }
            molgfx::core::VisualAttributeRef::LegacyScalar(property) => format!(
                "VisualAttributeRef.legacy_scalar(AtomPropertyHandle({}, {}))",
                property.row(),
                property.generation()
            ),
        }
    }
}

/// One visual program and draw order attached to an exact row domain.
#[pyclass(name = "VisualDescriptor", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyVisualDescriptor(pub(crate) molgfx::core::VisualDescriptor);

impl From<&molgfx::core::VisualDescriptor> for PyVisualDescriptor {
    fn from(value: &molgfx::core::VisualDescriptor) -> Self {
        Self(value.clone())
    }
}

#[pymethods]
impl PyVisualDescriptor {
    /// Typed visual program and its mutable parameter block.
    #[getter]
    fn style(&self) -> PyVisualStyle {
        PyVisualStyle(self.0.style().clone())
    }

    /// Stable draw order among overlapping attachments.
    #[getter]
    fn order(&self) -> i32 {
        self.0.order()
    }

    fn __repr__(&self) -> String {
        format!(
            "VisualDescriptor(order={}, fingerprint={})",
            self.0.order(),
            self.0.style().program().fingerprint()
        )
    }
}

#[pymethods]
impl PyVisualProgram {
    /// Column slots the program reads, in declaration order.
    fn attributes(&self) -> Vec<PyVisualAttributeRef> {
        self.0
            .attributes()
            .iter()
            .copied()
            .map(Into::into)
            .collect()
    }
}

#[pymethods]
impl PyVisualProgramBuilder {
    /// Reads a scene-independent scalar column.
    fn scalar_column(&mut self, key: PyVisualColumnKey) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.scalar_column(key.0)).map(PyScalarExpr)
    }

    /// Reads a scene-independent category column as an exact scalar.
    fn category_column(&mut self, key: PyVisualColumnKey) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.category_column(key.0)).map(PyScalarExpr)
    }

    /// Reads a scene-independent vector column.
    fn vector_column(&mut self, key: PyVisualColumnKey) -> PyResult<PyVectorExpr> {
        visual(self.builder()?.vector_column(key.0)).map(PyVectorExpr)
    }

    /// Reads a scene-independent RGBA8 column.
    fn color_column(&mut self, key: PyVisualColumnKey) -> PyResult<PyColorExpr> {
        visual(self.builder()?.color_column(key.0)).map(PyColorExpr)
    }
}

#[pymethods]
impl PyScene {
    /// Resolves the visual descriptor attached to one exact row domain.
    fn domain_visual(&self, domain: PyRowDomain) -> Option<PyVisualDescriptor> {
        self.inner.domain_visual(domain.0).map(Into::into)
    }

    /// Iterates attached descriptors in deterministic domain order.
    fn domain_visuals(&self) -> Vec<(PyRowDomain, PyVisualDescriptor)> {
        self.inner
            .domain_visuals()
            .map(|(domain, descriptor)| (PyRowDomain(domain), descriptor.into()))
            .collect()
    }
}
