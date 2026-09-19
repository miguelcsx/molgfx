//! Residency accounting adapters: what is resident, and what the frame cost.
//!
//! These are readings, not authoring. A caller watches them to decide whether
//! more detail has arrived, whether the arena is stalling, or whether a frame
//! leaned on the proxy path — so every value is a plain count or byte total.

use super::engine::PyEngine;
use crate::semantic::PyResidencyTicket;
use pyo3::prelude::*;

/// Fixed paged-arena counters.
#[pyclass(name = "ArenaMetrics", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyArenaMetrics(pub(crate) molgfx::gpu::ArenaMetrics);

#[pymethods]
impl PyArenaMetrics {
    #[getter]
    fn resident_bytes(&self) -> u64 {
        self.0.resident_bytes
    }

    #[getter]
    fn peak_resident_bytes(&self) -> u64 {
        self.0.peak_resident_bytes
    }

    #[getter]
    fn requested_bytes(&self) -> u64 {
        self.0.requested_bytes
    }

    #[getter]
    fn allocated_bytes(&self) -> u64 {
        self.0.allocated_bytes
    }

    #[getter]
    fn allocations(&self) -> u64 {
        self.0.allocations
    }

    #[getter]
    fn allocation_stalls(&self) -> u64 {
        self.0.allocation_stalls
    }

    #[getter]
    fn stalled_bytes(&self) -> u64 {
        self.0.stalled_bytes
    }

    #[getter]
    fn host_allocation_events(&self) -> u64 {
        self.0.host_allocation_events
    }
}

/// Fixed upload-ring counters. Cumulative except the live gauges.
#[pyclass(name = "UploadMetrics", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyUploadMetrics(pub(crate) molgfx::gpu::UploadMetrics);

#[pymethods]
impl PyUploadMetrics {
    #[getter]
    fn bytes_staged(&self) -> u64 {
        self.0.bytes_staged
    }

    #[getter]
    fn bytes_submitted(&self) -> u64 {
        self.0.bytes_submitted
    }

    #[getter]
    fn bytes_retired(&self) -> u64 {
        self.0.bytes_retired
    }

    #[getter]
    fn bytes_cancelled(&self) -> u64 {
        self.0.bytes_cancelled
    }

    #[getter]
    fn epoch_bytes(&self) -> u64 {
        self.0.epoch_bytes
    }

    #[getter]
    fn in_flight_bytes(&self) -> u64 {
        self.0.in_flight_bytes
    }

    #[getter]
    fn peak_in_flight_bytes(&self) -> u64 {
        self.0.peak_in_flight_bytes
    }

    #[getter]
    fn occupied_bytes(&self) -> u64 {
        self.0.occupied_bytes
    }

    #[getter]
    fn peak_occupied_bytes(&self) -> u64 {
        self.0.peak_occupied_bytes
    }

    #[getter]
    fn active_tickets(&self) -> u64 {
        self.0.active_tickets
    }

    #[getter]
    fn stall_events(&self) -> u64 {
        self.0.stall_events
    }

    #[getter]
    fn stalled_bytes(&self) -> u64 {
        self.0.stalled_bytes
    }

    #[getter]
    fn host_allocation_events(&self) -> u64 {
        self.0.host_allocation_events
    }
}

/// Everything the residency layer reports about the paged arenas.
#[pyclass(name = "ChunkResidencyMetrics", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyChunkResidencyMetrics(pub(crate) molgfx::render::ChunkResidencyMetrics);

#[pymethods]
impl PyChunkResidencyMetrics {
    #[getter]
    fn arena(&self) -> PyArenaMetrics {
        PyArenaMetrics(self.0.arena)
    }

    #[getter]
    fn uploads(&self) -> PyUploadMetrics {
        PyUploadMetrics(self.0.uploads)
    }

    #[getter]
    fn tracked_chunks(&self) -> usize {
        self.0.tracked_chunks
    }

    #[getter]
    fn tracked_capacity(&self) -> usize {
        self.0.tracked_capacity
    }
}

/// A structure chunk generation resident in the paged structure arena.
#[pyclass(name = "ResidentStructureChunk", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyResidentStructureChunk(pub(crate) molgfx::render::ResidentStructureChunk);

#[pymethods]
impl PyResidentStructureChunk {
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
    fn cluster_offset(&self) -> u32 {
        self.0.cluster_offset
    }

    #[getter]
    fn cluster_count(&self) -> u32 {
        self.0.cluster_count
    }
}

/// A provider trajectory frame resident in the bounded frame arena.
#[pyclass(name = "ResidentTrajectoryChunk", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyResidentTrajectoryChunk(pub(crate) molgfx::render::ResidentTrajectoryChunk);

#[pymethods]
impl PyResidentTrajectoryChunk {
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
}

/// Current and peak charges against the recomputable-data budget.
#[pyclass(name = "DerivedCacheUsage", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyDerivedCacheUsage(pub(crate) molgfx::render::DerivedCacheUsage);

#[pymethods]
impl PyDerivedCacheUsage {
    #[getter]
    fn cpu_bytes(&self) -> u64 {
        self.0.cpu_bytes
    }

    #[getter]
    fn gpu_bytes(&self) -> u64 {
        self.0.gpu_bytes
    }

    #[getter]
    fn peak_cpu_bytes(&self) -> u64 {
        self.0.peak_cpu_bytes
    }

    #[getter]
    fn peak_gpu_bytes(&self) -> u64 {
        self.0.peak_gpu_bytes
    }
}

/// Whether every requested resource contributed at full fidelity.
#[pyclass(name = "FrameCompleteness", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PyFrameCompleteness(pub(crate) molgfx::render::FrameCompleteness);

#[pymethods]
impl PyFrameCompleteness {
    /// No provider or upload work remains pending.
    #[classattr]
    #[pyo3(name = "Complete")]
    fn complete_variant() -> Self {
        Self(molgfx::render::FrameCompleteness::Complete)
    }

    /// A valid frame while more resident detail is still arriving.
    #[staticmethod]
    fn progressive(pending_chunks: u64) -> Self {
        Self(molgfx::render::FrameCompleteness::Progressive { pending_chunks })
    }

    #[getter]
    fn is_complete(&self) -> bool {
        matches!(self.0, molgfx::render::FrameCompleteness::Complete)
    }

    #[getter]
    fn pending_chunks(&self) -> u64 {
        match self.0 {
            molgfx::render::FrameCompleteness::Complete => 0,
            molgfx::render::FrameCompleteness::Progressive { pending_chunks } => pending_chunks,
        }
    }

    fn __repr__(&self) -> String {
        match self.0 {
            molgfx::render::FrameCompleteness::Complete => "FrameCompleteness.Complete".to_owned(),
            molgfx::render::FrameCompleteness::Progressive { pending_chunks } => {
                format!("FrameCompleteness.Progressive({pending_chunks})")
            }
        }
    }
}

/// The explicit approximations one frame used.
#[pyclass(name = "FrameDegradation", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyFrameDegradation(pub(crate) molgfx::render::FrameDegradation);

#[pymethods]
impl PyFrameDegradation {
    /// Non-resident detail was represented by the paged proxy path.
    #[classattr]
    #[pyo3(name = "STREAMING_PROXY")]
    fn streaming_proxy() -> Self {
        Self(molgfx::render::FrameDegradation::STREAMING_PROXY)
    }

    /// True when every bit of `feature` is active.
    fn contains(&self, feature: PyFrameDegradation) -> bool {
        self.0.contains(feature.0)
    }

    fn __repr__(&self) -> String {
        format!(
            "FrameDegradation(streaming_proxy={})",
            self.contains(PyFrameDegradation(
                molgfx::render::FrameDegradation::STREAMING_PROXY
            ))
        )
    }
}

/// Counters captured with one frame report.
#[pyclass(name = "FrameMetrics", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyFrameMetrics(pub(crate) molgfx::render::FrameMetrics);

#[pymethods]
impl PyFrameMetrics {
    #[getter]
    fn tracked_chunks(&self) -> usize {
        self.0.tracked_chunks
    }

    #[getter]
    fn upload_in_flight_bytes(&self) -> u64 {
        self.0.upload_in_flight_bytes
    }

    #[getter]
    fn derived_cache_gpu_bytes(&self) -> u64 {
        self.0.derived_cache_gpu_bytes
    }

    #[getter]
    fn derived_cache_peak_gpu_bytes(&self) -> u64 {
        self.0.derived_cache_peak_gpu_bytes
    }

    #[getter]
    fn physical_buffer_bytes(&self) -> u64 {
        self.0.physical_buffer_bytes
    }

    #[getter]
    fn physical_texture_bytes(&self) -> u64 {
        self.0.physical_texture_bytes
    }

    #[getter]
    fn physical_total_bytes(&self) -> u64 {
        self.0.physical_total_bytes
    }

    #[getter]
    fn physical_peak_bytes(&self) -> u64 {
        self.0.physical_peak_bytes
    }
}

#[pymethods]
impl PyEngine {
    /// Paged-arena and upload-ring counters, read without scanning the scene.
    fn chunk_residency_metrics(&self) -> PyChunkResidencyMetrics {
        PyChunkResidencyMetrics(self.inner.chunk_residency_metrics())
    }

    /// The resident structure generation for one ticket, when it is present.
    fn resident_structure_chunk(
        &self,
        ticket: PyResidencyTicket,
    ) -> Option<PyResidentStructureChunk> {
        self.inner
            .resident_structure_chunk(ticket.0)
            .map(PyResidentStructureChunk)
    }

    /// The resident provider frame for one ticket, when it is present.
    fn resident_trajectory_chunk(
        &self,
        ticket: PyResidencyTicket,
    ) -> Option<PyResidentTrajectoryChunk> {
        self.inner
            .resident_trajectory_chunk(ticket.0)
            .map(PyResidentTrajectoryChunk)
    }

    /// Current and peak recomputable-cache charges.
    fn derived_cache_usage(&self) -> PyDerivedCacheUsage {
        PyDerivedCacheUsage(self.inner.derived_cache_usage())
    }
}
