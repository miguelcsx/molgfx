//! Declarative placement adapters for the paged chunk path.
//!
//! A placement names one independently mutable occurrence of resident data:
//! which generation to draw, where it sits, and how it is drawn. The values are
//! built here and validated by the engine when the whole set is replaced, so a
//! rejected set says why once, with the identity that caused it.

use super::chunk_streaming::PyChunkPlacementStatus;
use super::engine::PyEngine;
use crate::core::representation::PyRelationStyle;
use crate::error::chunk_placement;
use crate::math::{PyMat4, PyRgba8};
use crate::semantic::PyResidencyTicket;
use pyo3::prelude::*;

/// Stable caller-owned identity for one independently mutable chunk occurrence.
#[pyclass(name = "ChunkPlacementId", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PyChunkPlacementId(pub(crate) molgfx::render::ChunkPlacementId);

#[pymethods]
impl PyChunkPlacementId {
    #[new]
    fn new(value: u64) -> Self {
        Self(molgfx::render::ChunkPlacementId::new(value))
    }

    #[getter]
    fn value(&self) -> u64 {
        self.0.get()
    }

    fn __repr__(&self) -> String {
        format!("ChunkPlacementId({})", self.0.get())
    }
}

impl From<molgfx::render::ChunkPlacementId> for PyChunkPlacementId {
    fn from(value: molgfx::render::ChunkPlacementId) -> Self {
        Self(value)
    }
}

/// How a resident chunk is drawn by the paged path.
#[pyclass(name = "ChunkRepresentation", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyChunkRepresentation(pub(crate) molgfx::render::ChunkRepresentation);

#[pymethods]
impl PyChunkRepresentation {
    /// Pixel-stable circular points, batched across every resident chunk.
    #[staticmethod]
    fn points(diameter_pixels: f32, color: PyRgba8) -> PyResult<Self> {
        chunk_placement(molgfx::render::ChunkRepresentation::points(
            diameter_pixels,
            color.0,
        ))
        .map(Self)
    }

    /// Analytic van der Waals spheres over provider-owned element radii.
    #[staticmethod]
    fn spacefill(radius_scale: f32, color: PyRgba8) -> PyResult<Self> {
        chunk_placement(molgfx::render::ChunkRepresentation::spacefill(
            radius_scale,
            color.0,
        ))
        .map(Self)
    }

    #[getter]
    fn kind(&self) -> &'static str {
        match self.0 {
            molgfx::render::ChunkRepresentation::Points { .. } => "points",
            molgfx::render::ChunkRepresentation::Spacefill { .. } => "spacefill",
        }
    }

    #[getter]
    fn diameter_pixels(&self) -> Option<f32> {
        match self.0 {
            molgfx::render::ChunkRepresentation::Points {
                diameter_pixels, ..
            } => Some(diameter_pixels),
            molgfx::render::ChunkRepresentation::Spacefill { .. } => None,
        }
    }

    #[getter]
    fn radius_scale(&self) -> Option<f32> {
        match self.0 {
            molgfx::render::ChunkRepresentation::Spacefill { radius_scale, .. } => {
                Some(radius_scale)
            }
            molgfx::render::ChunkRepresentation::Points { .. } => None,
        }
    }

    #[getter]
    fn color(&self) -> PyRgba8 {
        let color = match self.0 {
            molgfx::render::ChunkRepresentation::Points { color, .. }
            | molgfx::render::ChunkRepresentation::Spacefill { color, .. } => color,
        };
        PyRgba8(color)
    }

    fn __repr__(&self) -> String {
        format!("ChunkRepresentation.{}", self.kind())
    }
}

/// One transform and representation over a resident structure generation.
#[pyclass(name = "StructureChunkPlacement", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyStructureChunkPlacement(pub(crate) molgfx::render::StructureChunkPlacement);

#[pymethods]
impl PyStructureChunkPlacement {
    #[new]
    fn new(
        id: PyChunkPlacementId,
        ticket: PyResidencyTicket,
        model_to_world: PyMat4,
        representation: PyChunkRepresentation,
    ) -> Self {
        Self(molgfx::render::StructureChunkPlacement {
            id: id.0,
            ticket: ticket.0,
            model_to_world: model_to_world.0,
            representation: representation.0,
        })
    }

    #[getter]
    fn id(&self) -> PyChunkPlacementId {
        PyChunkPlacementId(self.0.id)
    }

    #[getter]
    fn ticket(&self) -> PyResidencyTicket {
        PyResidencyTicket(self.0.ticket)
    }

    #[getter]
    fn model_to_world(&self) -> PyMat4 {
        PyMat4(self.0.model_to_world)
    }

    #[getter]
    fn representation(&self) -> PyChunkRepresentation {
        PyChunkRepresentation(self.0.representation)
    }
}

/// Analytic cylinder placement over a resident provider bond chunk.
#[pyclass(name = "BondChunkPlacement", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyBondChunkPlacement(pub(crate) molgfx::render::BondChunkPlacement);

#[pymethods]
impl PyBondChunkPlacement {
    /// A licorice-style cylinder; ball-and-stick pairs it with space-filling
    /// atom placements over the same coordinate arena.
    #[staticmethod]
    fn licorice(
        id: PyChunkPlacementId,
        ticket: PyResidencyTicket,
        model_to_world: PyMat4,
        radius: f32,
        color: PyRgba8,
    ) -> PyResult<Self> {
        chunk_placement(molgfx::render::BondChunkPlacement::licorice(
            id.0,
            ticket.0,
            model_to_world.0,
            radius,
            color.0,
        ))
        .map(Self)
    }

    #[getter]
    fn id(&self) -> PyChunkPlacementId {
        PyChunkPlacementId(self.0.id)
    }

    #[getter]
    fn ticket(&self) -> PyResidencyTicket {
        PyResidencyTicket(self.0.ticket)
    }

    #[getter]
    fn model_to_world(&self) -> PyMat4 {
        PyMat4(self.0.model_to_world)
    }

    #[getter]
    fn radius(&self) -> f32 {
        self.0.radius
    }

    #[getter]
    fn color(&self) -> PyRgba8 {
        PyRgba8(self.0.color)
    }
}

/// Stroke fallback for one globally anchored resident relation chunk.
#[pyclass(name = "RelationChunkPlacement", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyRelationChunkPlacement(pub(crate) molgfx::render::RelationChunkPlacement);

#[pymethods]
impl PyRelationChunkPlacement {
    #[new]
    fn new(
        id: PyChunkPlacementId,
        ticket: PyResidencyTicket,
        style: PyRelationStyle,
    ) -> PyResult<Self> {
        chunk_placement(molgfx::render::RelationChunkPlacement::new(
            id.0, ticket.0, style.0,
        ))
        .map(Self)
    }

    #[getter]
    fn id(&self) -> PyChunkPlacementId {
        PyChunkPlacementId(self.0.id())
    }

    #[getter]
    fn ticket(&self) -> PyResidencyTicket {
        PyResidencyTicket(self.0.ticket())
    }

    #[getter]
    fn style(&self) -> PyRelationStyle {
        PyRelationStyle(self.0.style())
    }
}

/// Two-frame window sampled by one animated rigid-instance placement.
#[pyclass(name = "InstanceChunkWindow", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyInstanceChunkWindow(pub(crate) molgfx::render::InstanceChunkWindow);

#[pymethods]
impl PyInstanceChunkWindow {
    #[new]
    fn new(
        placement: PyChunkPlacementId,
        start: PyResidencyTicket,
        end: PyResidencyTicket,
        interpolation: f32,
    ) -> PyResult<Self> {
        chunk_placement(molgfx::render::InstanceChunkWindow::new(
            placement.0,
            start.0,
            end.0,
            interpolation,
        ))
        .map(Self)
    }

    #[getter]
    fn placement(&self) -> PyChunkPlacementId {
        PyChunkPlacementId(self.0.placement)
    }

    #[getter]
    fn start(&self) -> PyResidencyTicket {
        PyResidencyTicket(self.0.start)
    }

    #[getter]
    fn end(&self) -> PyResidencyTicket {
        PyResidencyTicket(self.0.end)
    }

    #[getter]
    fn interpolation(&self) -> f32 {
        self.0.interpolation
    }
}

/// Two-frame window sampled by one scene-independent attribute column.
#[pyclass(name = "AttributeChunkWindow", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyAttributeChunkWindow(pub(crate) molgfx::render::AttributeChunkWindow);

#[pymethods]
impl PyAttributeChunkWindow {
    #[new]
    fn new(
        attribute: PyResidencyTicket,
        start: PyResidencyTicket,
        end: PyResidencyTicket,
        interpolation: f32,
    ) -> PyResult<Self> {
        chunk_placement(molgfx::render::AttributeChunkWindow::new(
            attribute.0,
            start.0,
            end.0,
            interpolation,
        ))
        .map(Self)
    }

    #[getter]
    fn attribute(&self) -> PyResidencyTicket {
        PyResidencyTicket(self.0.attribute)
    }

    #[getter]
    fn start(&self) -> PyResidencyTicket {
        PyResidencyTicket(self.0.start)
    }

    #[getter]
    fn end(&self) -> PyResidencyTicket {
        PyResidencyTicket(self.0.end)
    }

    #[getter]
    fn interpolation(&self) -> f32 {
        self.0.interpolation
    }
}

#[pymethods]
impl PyEngine {
    /// Replaces every structure placement in one bounded operation.
    fn set_structure_chunk_placements(
        &mut self,
        placements: Vec<PyStructureChunkPlacement>,
    ) -> PyResult<()> {
        let placements: Vec<molgfx::render::StructureChunkPlacement> =
            placements.into_iter().map(|value| value.0).collect();
        crate::error::chunk_residency(self.inner.set_structure_chunk_placements(&placements))
    }

    fn structure_chunk_placement_status(&self, id: PyChunkPlacementId) -> PyChunkPlacementStatus {
        self.inner.structure_chunk_placement_status(id.0).into()
    }

    /// Replaces every analytic bond placement in one bounded operation.
    fn set_bond_chunk_placements(&mut self, placements: Vec<PyBondChunkPlacement>) -> PyResult<()> {
        let placements: Vec<molgfx::render::BondChunkPlacement> =
            placements.into_iter().map(|value| value.0).collect();
        crate::error::chunk_residency(self.inner.set_bond_chunk_placements(&placements))
    }

    fn bond_chunk_placement_status(&self, id: PyChunkPlacementId) -> PyChunkPlacementStatus {
        self.inner.bond_chunk_placement_status(id.0).into()
    }

    /// Replaces every globally anchored relation occurrence.
    fn set_relation_chunk_placements(
        &mut self,
        placements: Vec<PyRelationChunkPlacement>,
    ) -> PyResult<()> {
        let placements: Vec<molgfx::render::RelationChunkPlacement> =
            placements.into_iter().map(|value| value.0).collect();
        crate::error::chunk_residency(self.inner.set_relation_chunk_placements(&placements))
    }

    fn relation_chunk_placement_status(&self, id: PyChunkPlacementId) -> PyChunkPlacementStatus {
        self.inner.relation_chunk_placement_status(id.0).into()
    }

    /// Replaces the two-frame windows sampled by animated instances.
    fn set_instance_chunk_windows(&mut self, windows: Vec<PyInstanceChunkWindow>) -> PyResult<()> {
        let windows: Vec<molgfx::render::InstanceChunkWindow> =
            windows.into_iter().map(|value| value.0).collect();
        crate::error::chunk_residency(self.inner.set_instance_chunk_windows(&windows))
    }

    /// Replaces the two-frame windows of scene-independent attribute columns.
    fn set_attribute_chunk_windows(
        &mut self,
        windows: Vec<PyAttributeChunkWindow>,
    ) -> PyResult<()> {
        let windows: Vec<molgfx::render::AttributeChunkWindow> =
            windows.into_iter().map(|value| value.0).collect();
        crate::error::chunk_residency(self.inner.set_attribute_chunk_windows(&windows))
    }
}
