//! Contiguous batch adapters for generic paged engine residency.

use super::chunk_placement::PyChunkPlacementId;
use super::engine::PyEngine;
use crate::core::PyAnalyticTemplate;
use crate::error::{chunk_residency, render, value};
use crate::math::{PyMat4, PyRgba8};
use crate::semantic::{PyDatasetCatalog, PyResidencyRequest, PyResidencyTicket};
use numpy::{PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "PointChunkPlacement", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyPointChunkPlacement(molgfx::render::PointChunkPlacement);

#[pymethods]
impl PyPointChunkPlacement {
    #[new]
    fn new(
        id: PyChunkPlacementId,
        ticket: PyResidencyTicket,
        model_to_world: PyMat4,
        diameter_pixels: f32,
        color: PyRgba8,
    ) -> PyResult<Self> {
        molgfx::render::PointChunkPlacement::new(
            id.0,
            ticket.0,
            model_to_world.0,
            diameter_pixels,
            color.0,
        )
        .map(Self)
        .map_err(|error| value(error.to_string()))
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
    fn model_to_world(&self) -> PyMat4 {
        PyMat4(self.0.model_to_world())
    }

    #[getter]
    fn diameter_pixels(&self) -> f32 {
        self.0.diameter_pixels()
    }

    #[getter]
    fn color(&self) -> PyRgba8 {
        PyRgba8(self.0.color())
    }
}

#[pyclass(name = "InstanceChunkPlacement", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyInstanceChunkPlacement(molgfx::render::InstanceChunkPlacement);

#[pymethods]
impl PyInstanceChunkPlacement {
    #[new]
    fn new(
        id: PyChunkPlacementId,
        ticket: PyResidencyTicket,
        template: &PyAnalyticTemplate,
        color: PyRgba8,
    ) -> Self {
        Self(molgfx::render::InstanceChunkPlacement::new(
            id.0,
            ticket.0,
            template.native(),
            color.0,
        ))
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
    fn color(&self) -> PyRgba8 {
        PyRgba8(self.0.color())
    }
}

#[pyclass(name = "ChunkPlacementStatus", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyChunkPlacementStatus {
    Missing,
    NotResident,
    Resident,
}

impl From<molgfx::render::ChunkPlacementStatus> for PyChunkPlacementStatus {
    fn from(value: molgfx::render::ChunkPlacementStatus) -> Self {
        match value {
            molgfx::render::ChunkPlacementStatus::Missing => Self::Missing,
            molgfx::render::ChunkPlacementStatus::NotResident => Self::NotResident,
            molgfx::render::ChunkPlacementStatus::Resident => Self::Resident,
        }
    }
}

#[pyclass(name = "ResidentGenericChunk", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyResidentGenericChunk(molgfx::render::ResidentGenericChunk);

#[pymethods]
impl PyResidentGenericChunk {
    #[getter]
    fn ticket(&self) -> PyResidencyTicket {
        PyResidencyTicket(self.0.ticket)
    }

    #[getter]
    fn byte_offset(&self) -> u64 {
        self.0.byte_offset
    }

    #[getter]
    fn byte_len(&self) -> u64 {
        self.0.byte_len
    }

    #[getter]
    fn local_rows(&self) -> u32 {
        self.0.local_rows
    }

    #[getter]
    fn stride(&self) -> u32 {
        self.0.stride
    }
}

#[pymethods]
impl PyEngine {
    /// Requests chunk generations in one Python crossing.
    fn request_chunks(
        &mut self,
        py: Python<'_>,
        requests: Vec<PyResidencyRequest>,
    ) -> PyResult<Vec<PyResidencyTicket>> {
        py.detach(|| {
            let mut tickets = Vec::with_capacity(requests.len());
            for request in requests {
                let ticket = render(
                    self.inner
                        .request_chunk_into(request.0, &mut self.residency_output)
                        .map_err(molgfx::render::RenderError::from),
                )?;
                tickets.push(PyResidencyTicket(ticket));
            }
            Ok(tickets)
        })
    }

    /// Copies one C-contiguous `(rows, 3)` float32 array into shared Rust storage.
    fn deliver_point_chunk(
        &mut self,
        py: Python<'_>,
        catalog: &PyDatasetCatalog,
        ticket: PyResidencyTicket,
        positions: PyReadonlyArray2<'_, f32>,
    ) -> PyResult<()> {
        let shape = positions.shape();
        if shape.len() != 2 || shape[1] != 3 {
            return Err(value("point positions must have shape (rows, 3)"));
        }
        let values = positions
            .as_slice()
            .map_err(|_| value("point positions must be C-contiguous float32"))?;
        let rows: &[[f32; 3]] = bytemuck::cast_slice(values);
        let payload = molgfx::core::PointChunkPayload::new(Arc::from(rows))
            .map_err(|error| value(error.to_string()))?;
        let data = molgfx::core::ChunkData::new(
            &catalog.0,
            ticket.0.key.chunk,
            molgfx::core::ChunkPayload::PointBatch(payload),
        )
        .map_err(|error| value(error.to_string()))?;
        py.detach(|| {
            render(
                self.inner
                    .deliver_chunk_into(ticket.0, data, &mut self.residency_output)
                    .map_err(molgfx::render::RenderError::from),
            )
        })
    }

    /// Validates one C-contiguous `(rows, 8)` rigid-transform array in Rust.
    fn deliver_instance_chunk(
        &mut self,
        py: Python<'_>,
        catalog: &PyDatasetCatalog,
        ticket: PyResidencyTicket,
        transforms: PyReadonlyArray2<'_, f32>,
    ) -> PyResult<()> {
        let shape = transforms.shape();
        if shape.len() != 2 || shape[1] != 8 {
            return Err(value("instance transforms must have shape (rows, 8)"));
        }
        let values = transforms
            .as_slice()
            .map_err(|_| value("instance transforms must be C-contiguous float32"))?;
        let mut instances = Vec::with_capacity(shape[0]);
        for row in values.chunks_exact(8) {
            instances.push(
                molgfx::core::RigidInstance::new(
                    molgfx::math::Vec3::new(row[0], row[1], row[2]),
                    molgfx::math::Quat::from_array([row[4], row[5], row[6], row[7]]),
                    row[3],
                )
                .map_err(|error| value(error.to_string()))?,
            );
        }
        let payload = molgfx::core::InstanceChunkPayload::new(Arc::from(instances))
            .map_err(|error| value(error.to_string()))?;
        let data = molgfx::core::ChunkData::new(
            &catalog.0,
            ticket.0.key.chunk,
            molgfx::core::ChunkPayload::InstanceBatch(payload),
        )
        .map_err(|error| value(error.to_string()))?;
        py.detach(|| {
            render(
                self.inner
                    .deliver_chunk_into(ticket.0, data, &mut self.residency_output)
                    .map_err(molgfx::render::RenderError::from),
            )
        })
    }

    /// Stages multiple already-delivered chunks without per-row Python calls.
    fn upload_chunks(&mut self, py: Python<'_>, tickets: Vec<PyResidencyTicket>) -> PyResult<()> {
        py.detach(|| {
            for ticket in tickets {
                render(
                    self.inner
                        .upload_chunk_into(ticket.0, &mut self.residency_output)
                        .map_err(molgfx::render::RenderError::from),
                )?;
            }
            Ok(())
        })
    }

    /// Publishes only backend-signalled uploads.
    fn poll_chunk_uploads(&mut self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            render(
                self.inner
                    .poll_chunk_uploads_into(&mut self.residency_output)
                    .map_err(molgfx::render::RenderError::from),
            )
        })
    }

    /// Replaces all point-chunk placements in one bounded operation.
    fn set_point_chunk_placements(
        &mut self,
        placements: Vec<PyPointChunkPlacement>,
    ) -> PyResult<()> {
        let placements: Vec<molgfx::render::PointChunkPlacement> = placements
            .into_iter()
            .map(|placement| placement.0)
            .collect();
        chunk_residency(self.inner.set_point_chunk_placements(&placements))
    }

    fn point_chunk_placement_status(&self, id: PyChunkPlacementId) -> PyChunkPlacementStatus {
        self.inner.point_chunk_placement_status(id.0).into()
    }

    fn set_instance_chunk_placements(
        &mut self,
        placements: Vec<PyInstanceChunkPlacement>,
    ) -> PyResult<()> {
        let placements: Vec<molgfx::render::InstanceChunkPlacement> = placements
            .into_iter()
            .map(|placement| placement.0)
            .collect();
        chunk_residency(self.inner.set_instance_chunk_placements(&placements))
    }

    fn instance_chunk_placement_status(&self, id: PyChunkPlacementId) -> PyChunkPlacementStatus {
        self.inner.instance_chunk_placement_status(id.0).into()
    }

    fn resident_generic_chunk(&self, ticket: PyResidencyTicket) -> Option<PyResidentGenericChunk> {
        self.inner
            .resident_generic_chunk(ticket.0)
            .map(PyResidentGenericChunk)
    }
}
