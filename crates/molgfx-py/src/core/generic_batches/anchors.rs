//! Relation anchor identities and the layouts their endpoints lower to.

use super::super::PyInstanceBatchHandle;
use super::model::PyRowDomain;
use crate::error::core;
use crate::math::PyVec3;
use pyo3::prelude::*;

#[pyclass(name = "RowEntityRef", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyRowEntityRef(pub(crate) molgfx::core::RowEntityRef);

impl From<molgfx::core::RowEntityRef> for PyRowEntityRef {
    fn from(value: molgfx::core::RowEntityRef) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyRowEntityRef {
    #[new]
    fn new(domain: PyRowDomain, row: u32) -> Self {
        Self(molgfx::core::RowEntityRef::new(domain.0, row))
    }

    #[getter]
    fn domain(&self) -> PyRowDomain {
        PyRowDomain(self.0.domain())
    }

    #[getter]
    fn row(&self) -> u32 {
        self.0.row()
    }
}

/// Exact occurrence of one shared template part placed by one rigid instance.
#[pyclass(name = "TemplatePartRef", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyTemplatePartRef(pub(crate) molgfx::core::TemplatePartRef);

impl From<molgfx::core::TemplatePartRef> for PyTemplatePartRef {
    fn from(value: molgfx::core::TemplatePartRef) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyTemplatePartRef {
    #[new]
    fn new(batch: PyInstanceBatchHandle, instance_row: u32, part_row: u32) -> Self {
        Self(molgfx::core::TemplatePartRef::new(
            batch.0,
            instance_row,
            part_row,
        ))
    }

    /// Shared instance batch holding the template.
    #[getter]
    fn batch(&self) -> PyInstanceBatchHandle {
        self.0.batch().into()
    }

    /// Transform row placing the shared template.
    #[getter]
    fn instance_row(&self) -> u32 {
        self.0.instance_row()
    }

    /// Sphere-first local template part row.
    #[getter]
    fn part_row(&self) -> u32 {
        self.0.part_row()
    }
}

/// Endpoint layout used to select one branch-free relation kernel.
#[pyclass(name = "AnchorLayout", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyAnchorLayout {
    World,
    Atom,
    Point,
    Instance,
    TemplatePart,
}

impl From<PyAnchorLayout> for molgfx::core::AnchorLayout {
    fn from(value: PyAnchorLayout) -> Self {
        match value {
            PyAnchorLayout::World => Self::World,
            PyAnchorLayout::Atom => Self::Atom,
            PyAnchorLayout::Point => Self::Point,
            PyAnchorLayout::Instance => Self::Instance,
            PyAnchorLayout::TemplatePart => Self::TemplatePart,
        }
    }
}

impl From<molgfx::core::AnchorLayout> for PyAnchorLayout {
    fn from(value: molgfx::core::AnchorLayout) -> Self {
        match value {
            molgfx::core::AnchorLayout::World => Self::World,
            molgfx::core::AnchorLayout::Atom => Self::Atom,
            molgfx::core::AnchorLayout::Point => Self::Point,
            molgfx::core::AnchorLayout::Instance => Self::Instance,
            molgfx::core::AnchorLayout::TemplatePart => Self::TemplatePart,
        }
    }
}

/// Ordered start and end layouts of one homogeneous relation stream.
#[pyclass(name = "RelationLayout", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyRelationLayout(pub(crate) molgfx::core::RelationLayout);

impl From<molgfx::core::RelationLayout> for PyRelationLayout {
    fn from(value: molgfx::core::RelationLayout) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyRelationLayout {
    #[new]
    fn new(start: PyAnchorLayout, end: PyAnchorLayout) -> Self {
        Self(molgfx::core::RelationLayout {
            start: start.into(),
            end: end.into(),
        })
    }

    #[getter]
    fn start(&self) -> PyAnchorLayout {
        self.0.start.into()
    }

    #[getter]
    fn end(&self) -> PyAnchorLayout {
        self.0.end.into()
    }

    /// Whether this stream needs endpoint resolution after state changes.
    #[getter]
    fn is_dynamic(&self) -> bool {
        self.0.is_dynamic()
    }

    fn __repr__(&self) -> String {
        format!(
            "RelationLayout(start={:?}, end={:?})",
            self.0.start, self.0.end
        )
    }
}

/// Spatial endpoint of a relation: a world position, an entity row or a
/// template-part occurrence.
#[pyclass(name = "SpatialAnchor", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PySpatialAnchor(pub(crate) molgfx::core::SpatialAnchor);

impl From<molgfx::core::SpatialAnchor> for PySpatialAnchor {
    fn from(value: molgfx::core::SpatialAnchor) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PySpatialAnchor {
    /// A finite caller-supplied world position.
    #[staticmethod]
    fn world(position: PyVec3) -> PyResult<Self> {
        core(molgfx::core::SpatialAnchor::world(position.0)).map(Self)
    }

    /// A dynamic endpoint on one row of a spatial domain.
    #[staticmethod]
    fn entity(entity: PyRowEntityRef) -> PyResult<Self> {
        core(molgfx::core::SpatialAnchor::entity(entity.0)).map(Self)
    }

    /// An exact occurrence of one shared template part.
    #[staticmethod]
    fn template_part(reference: PyTemplatePartRef) -> Self {
        Self(molgfx::core::SpatialAnchor::template_part(reference.0))
    }

    /// The world position of a `world` anchor.
    #[getter]
    fn position(&self) -> Option<PyVec3> {
        match self.0 {
            molgfx::core::SpatialAnchor::World(position) => Some(PyVec3(position)),
            _ => None,
        }
    }

    /// The referenced row of an `entity` anchor.
    #[getter]
    fn source_entity(&self) -> Option<PyRowEntityRef> {
        match self.0 {
            molgfx::core::SpatialAnchor::Entity(entity) => Some(PyRowEntityRef(entity)),
            _ => None,
        }
    }

    /// The occurrence of a `template_part` anchor.
    #[getter]
    fn reference(&self) -> Option<PyTemplatePartRef> {
        match self.0 {
            molgfx::core::SpatialAnchor::TemplatePart(reference) => {
                Some(PyTemplatePartRef(reference))
            }
            _ => None,
        }
    }

    /// Homogeneous lowering layout.
    #[getter]
    fn layout(&self) -> PyAnchorLayout {
        self.0.layout().into()
    }

    /// Whether coordinates, transforms or timeline can move this endpoint.
    #[getter]
    fn is_dynamic(&self) -> bool {
        self.0.is_dynamic()
    }

    /// Spatial source table used by branch-free lowering.
    #[getter]
    fn source_domain(&self) -> Option<PyRowDomain> {
        self.0.source_domain().map(PyRowDomain)
    }

    fn __repr__(&self) -> String {
        match self.0 {
            molgfx::core::SpatialAnchor::World(position) => {
                format!(
                    "SpatialAnchor.world(Vec3({}, {}, {}))",
                    position.x, position.y, position.z
                )
            }
            molgfx::core::SpatialAnchor::Entity(entity) => format!(
                "SpatialAnchor.entity(row={}, layout={:?})",
                entity.row(),
                self.0.layout()
            ),
            molgfx::core::SpatialAnchor::TemplatePart(reference) => format!(
                "SpatialAnchor.template_part(instance_row={}, part_row={})",
                reference.instance_row(),
                reference.part_row()
            ),
        }
    }
}
