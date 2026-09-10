//! Mechanical Python adapters for caller-driven sparse brick residency.

use super::brick::{PyBrickCatalog, PyBrickDescriptor, PyBrickId};
use crate::error::{brick_atlas, value};
use numpy::PyReadonlyArrayDyn;
use pyo3::prelude::*;

#[pyclass(name = "UploadRingConfig", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyUploadRingConfig(pub(crate) pdviewx::UploadRingConfig);

#[pymethods]
impl PyUploadRingConfig {
    #[new]
    fn new(
        capacity_bytes: usize,
        ticket_capacity: usize,
        epoch_budget_bytes: usize,
        in_flight_budget_bytes: usize,
        alignment: usize,
    ) -> Self {
        Self(pdviewx::UploadRingConfig {
            capacity_bytes,
            ticket_capacity,
            epoch_budget_bytes,
            in_flight_budget_bytes,
            alignment,
        })
    }

    #[getter]
    fn capacity_bytes(&self) -> usize {
        self.0.capacity_bytes
    }

    #[getter]
    fn ticket_capacity(&self) -> usize {
        self.0.ticket_capacity
    }

    #[getter]
    fn epoch_budget_bytes(&self) -> usize {
        self.0.epoch_budget_bytes
    }

    #[getter]
    fn in_flight_budget_bytes(&self) -> usize {
        self.0.in_flight_budget_bytes
    }

    #[getter]
    fn alignment(&self) -> usize {
        self.0.alignment
    }
}

#[pyclass(name = "BrickAtlasKind", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyBrickAtlasKind {
    Scalar,
    Segmentation,
    Occupancy,
    Surface,
}

impl From<PyBrickAtlasKind> for pdviewx::BrickAtlasKind {
    fn from(value: PyBrickAtlasKind) -> Self {
        match value {
            PyBrickAtlasKind::Scalar => Self::Scalar,
            PyBrickAtlasKind::Segmentation => Self::Segmentation,
            PyBrickAtlasKind::Occupancy => Self::Occupancy,
            PyBrickAtlasKind::Surface => Self::Surface,
        }
    }
}

#[pyclass(name = "BrickAtlasConfig", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyBrickAtlasConfig(pub(crate) pdviewx::BrickAtlasConfig);

#[pymethods]
impl PyBrickAtlasConfig {
    #[new]
    fn new(
        stored_shape: [u16; 3],
        resident_capacity: usize,
        uploads: PyUploadRingConfig,
        kind: PyBrickAtlasKind,
    ) -> Self {
        Self(pdviewx::BrickAtlasConfig {
            stored_shape,
            resident_capacity,
            uploads: uploads.0,
            kind: kind.into(),
        })
    }

    #[getter]
    fn stored_shape(&self) -> [u16; 3] {
        self.0.stored_shape
    }

    #[getter]
    fn resident_capacity(&self) -> usize {
        self.0.resident_capacity
    }

    #[getter]
    fn uploads(&self) -> PyUploadRingConfig {
        PyUploadRingConfig(self.0.uploads)
    }

    #[getter]
    fn kind(&self) -> PyBrickAtlasKind {
        match self.0.kind {
            pdviewx::BrickAtlasKind::Scalar => PyBrickAtlasKind::Scalar,
            pdviewx::BrickAtlasKind::Segmentation => PyBrickAtlasKind::Segmentation,
            pdviewx::BrickAtlasKind::Occupancy => PyBrickAtlasKind::Occupancy,
            pdviewx::BrickAtlasKind::Surface => PyBrickAtlasKind::Surface,
        }
    }
}

#[pyclass(name = "FenceValue", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PyFenceValue(pdviewx::FenceValue);

#[pymethods]
impl PyFenceValue {
    #[getter]
    fn value(&self) -> u64 {
        self.0.0
    }

    fn __int__(&self) -> u64 {
        self.0.0
    }
}

#[pyclass(name = "BrickAtlasMetrics", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyBrickAtlasMetrics(pdviewx::BrickAtlasMetrics);

#[pymethods]
impl PyBrickAtlasMetrics {
    #[getter]
    fn atlas_bytes(&self) -> u64 {
        self.0.atlas_bytes
    }

    #[getter]
    fn page_table_bytes(&self) -> u64 {
        self.0.page_table_bytes
    }

    #[getter]
    fn resident_pages(&self) -> usize {
        self.0.resident_pages
    }

    #[getter]
    fn pending_uploads(&self) -> usize {
        self.0.pending_uploads
    }

    #[getter]
    fn pending_evictions(&self) -> usize {
        self.0.pending_evictions
    }

    #[getter]
    fn stale_completions(&self) -> u64 {
        self.0.stale_completions
    }

    #[getter]
    fn page_table_writes(&self) -> u64 {
        self.0.page_table_writes
    }
}

pub(super) fn install(
    engine: &mut pdviewx::Engine,
    catalog: &PyBrickCatalog,
    config: PyBrickAtlasConfig,
) -> PyResult<usize> {
    brick_atlas(engine.install_brick_atlas(&catalog.0, config.0))
}

pub(super) fn stage(
    engine: &mut pdviewx::Engine,
    atlas: usize,
    descriptor: PyBrickDescriptor,
    bytes: PyReadonlyArrayDyn<'_, u8>,
) -> PyResult<PyFenceValue> {
    let bytes = borrowed_bytes(&bytes)?;
    brick_atlas(engine.stage_brick(
        atlas,
        pdviewx::BrickAtlasUpload {
            descriptor: descriptor.0,
            bytes,
        },
    ))
    .map(PyFenceValue)
}

fn borrowed_bytes<'a>(bytes: &'a PyReadonlyArrayDyn<'_, u8>) -> PyResult<&'a [u8]> {
    bytes
        .as_slice()
        .map_err(|_| value("brick bytes must be a C-contiguous uint8 array"))
}

pub(super) fn evict(
    engine: &mut pdviewx::Engine,
    atlas: usize,
    brick: PyBrickId,
) -> PyResult<PyFenceValue> {
    brick_atlas(engine.evict_brick(atlas, brick.0)).map(PyFenceValue)
}

pub(super) fn metrics(engine: &mut pdviewx::Engine, atlas: usize) -> Option<PyBrickAtlasMetrics> {
    engine.brick_atlas_metrics(atlas).map(PyBrickAtlasMetrics)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyUploadRingConfig>()?;
    module.add_class::<PyBrickAtlasKind>()?;
    module.add_class::<PyBrickAtlasConfig>()?;
    module.add_class::<PyFenceValue>()?;
    module.add_class::<PyBrickAtlasMetrics>()
}

#[cfg(test)]
#[path = "brick_atlas_tests.rs"]
mod tests;
