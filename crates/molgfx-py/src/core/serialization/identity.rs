//! Identity vocabulary shared by every manifest record.
//!
//! A manifest names things the way a second process can still resolve them: a
//! slot row with its generation, a row domain, an external source-row
//! namespace, and a world-space anchor that may carry the entity it came from.

use pyo3::prelude::*;

/// Stable generational identity for a scene-owned object.
#[pyclass(name = "ObjectIdentity", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PyObjectIdentity(pub(crate) molgfx::core::ObjectIdentity);

#[pymethods]
impl PyObjectIdentity {
    #[new]
    fn new(row: u32, generation: u32) -> Self {
        Self(molgfx::core::ObjectIdentity { row, generation })
    }

    /// Slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Slot generation.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    fn __repr__(&self) -> String {
        format!(
            "ObjectIdentity(row={}, generation={})",
            self.0.row, self.0.generation
        )
    }
}

impl From<molgfx::core::ObjectIdentity> for PyObjectIdentity {
    fn from(value: molgfx::core::ObjectIdentity) -> Self {
        Self(value)
    }
}

/// An entity identity carried by an annotation or interaction anchor.
#[pyclass(name = "EntityDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyEntityDescription(pub(crate) molgfx::core::EntityDescription);

#[pymethods]
impl PyEntityDescription {
    /// Owning structure identity.
    #[getter]
    fn structure(&self) -> PyObjectIdentity {
        PyObjectIdentity(self.0.structure)
    }

    /// Stable entity kind name.
    #[getter]
    fn kind(&self) -> String {
        self.0.kind.clone()
    }

    /// Source table row.
    #[getter]
    fn index(&self) -> u32 {
        self.0.index
    }
}

impl From<molgfx::core::EntityDescription> for PyEntityDescription {
    fn from(value: molgfx::core::EntityDescription) -> Self {
        Self(value)
    }
}

/// A world-space anchor with optional entity provenance.
#[pyclass(name = "AnchorDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyAnchorDescription(pub(crate) molgfx::core::AnchorDescription);

#[pymethods]
impl PyAnchorDescription {
    /// Position in ångström.
    #[getter]
    fn position(&self) -> [f32; 3] {
        self.0.position
    }

    /// Optional source entity.
    #[getter]
    fn entity(&self) -> Option<PyEntityDescription> {
        self.0.entity.clone().map(PyEntityDescription)
    }
}

impl From<molgfx::core::AnchorDescription> for PyAnchorDescription {
    fn from(value: molgfx::core::AnchorDescription) -> Self {
        Self(value)
    }
}

/// One structure-local selection mask.
#[pyclass(name = "SelectionMask", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PySelectionMask(pub(crate) molgfx::core::SelectionMask);

#[pymethods]
impl PySelectionMask {
    /// Placed-structure slot row.
    #[getter]
    fn structure_row(&self) -> u32 {
        self.0.structure_row
    }

    /// Selected atom rows in ascending order.
    #[getter]
    fn atoms(&self) -> Vec<u32> {
        self.0.atoms.clone()
    }
}

impl From<molgfx::core::SelectionMask> for PySelectionMask {
    fn from(value: molgfx::core::SelectionMask) -> Self {
        Self(value)
    }
}

/// A representation target expressed without process-local handles.
#[pyclass(name = "TargetDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyTargetDescription(pub(crate) molgfx::core::TargetDescription);

#[pymethods]
impl PyTargetDescription {
    #[new]
    fn new(kind: String, row: u32, generation: u32) -> Self {
        Self(molgfx::core::TargetDescription {
            kind,
            row,
            generation,
        })
    }

    /// `selection`, `volume` or `segmentation`.
    #[getter]
    fn kind(&self) -> String {
        self.0.kind.clone()
    }

    /// Target slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Target slot generation.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }
}

/// Process-independent identity of one exact row domain.
#[pyclass(name = "RowDomainDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyRowDomainDescription(pub(crate) molgfx::core::RowDomainDescription);

#[pymethods]
impl PyRowDomainDescription {
    /// `atoms`, `points`, `instances`, `template-parts` or `relations`.
    #[getter]
    fn kind(&self) -> String {
        self.0.kind.clone()
    }

    /// Generational table row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Generational table generation.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }
}

impl From<molgfx::core::RowDomainDescription> for PyRowDomainDescription {
    fn from(value: molgfx::core::RowDomainDescription) -> Self {
        Self(value)
    }
}

/// External source-row namespace without embedded key arrays.
#[pyclass(name = "SourceRowsDescription", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PySourceRowsDescription(pub(crate) molgfx::core::SourceRowsDescription);

#[pymethods]
impl PySourceRowsDescription {
    /// Caller-owned namespace.
    #[getter]
    fn namespace(&self) -> u64 {
        self.0.namespace
    }

    /// Logical row count.
    #[getter]
    fn row_count(&self) -> u32 {
        self.0.row_count
    }

    /// Whether the referenced payload carries explicit `u64` keys.
    #[getter]
    fn keyed(&self) -> bool {
        self.0.keyed
    }
}

impl From<molgfx::core::SourceRowsDescription> for PySourceRowsDescription {
    fn from(value: molgfx::core::SourceRowsDescription) -> Self {
        Self(value)
    }
}
