//! Thin Python values for paged dataset metadata.

use crate::error::dataset;
use pyo3::prelude::*;

#[cfg(test)]
#[path = "dataset_tests.rs"]
mod tests;

macro_rules! id_type {
    ($rust:ident, $python:literal, $native:ident, $value:ty) => {
        #[pyclass(name = $python, frozen, eq, from_py_object)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) struct $rust(pub(crate) molgfx::$native);

        #[pymethods]
        impl $rust {
            #[new]
            fn new(value: $value) -> Self {
                Self(molgfx::$native::new(value))
            }

            #[getter]
            fn value(&self) -> $value {
                self.0.get()
            }

            fn __int__(&self) -> $value {
                self.0.get()
            }

            fn __repr__(&self) -> String {
                format!("{}({})", $python, self.0.get())
            }
        }
    };
}

id_type!(PyDatasetId, "DatasetId", DatasetId, u64);
id_type!(PyChunkId, "ChunkId", ChunkId, u64);
id_type!(PyLogicalRow, "LogicalRow", LogicalRow, u64);
id_type!(PyLocalRow, "LocalRow", LocalRow, u32);

#[pyclass(name = "ChunkFootprint", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyChunkFootprint(pub(crate) molgfx::ChunkFootprint);

#[pymethods]
impl PyChunkFootprint {
    #[new]
    #[pyo3(signature = (source_bytes=0, host_bytes=0, staging_bytes=0, gpu_bytes=0))]
    fn new(source_bytes: u64, host_bytes: u64, staging_bytes: u64, gpu_bytes: u64) -> Self {
        Self(molgfx::ChunkFootprint::new(
            source_bytes,
            host_bytes,
            staging_bytes,
            gpu_bytes,
        ))
    }

    #[getter]
    fn source_bytes(&self) -> u64 {
        self.0.source_bytes
    }

    #[getter]
    fn host_bytes(&self) -> u64 {
        self.0.host_bytes
    }

    #[getter]
    fn staging_bytes(&self) -> u64 {
        self.0.staging_bytes
    }

    #[getter]
    fn gpu_bytes(&self) -> u64 {
        self.0.gpu_bytes
    }

    fn total_bytes(&self) -> PyResult<u64> {
        dataset(self.0.total_bytes())
    }
}

#[pyclass(name = "PayloadKind", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyPayloadKind {
    Structure,
    BondTopology,
    ScalarProperty,
    Trajectory,
    VolumeBrick,
    LabelBrick,
    Mesh,
    Proxy,
    PointBatch,
    InstanceBatch,
    RelationBatch,
    Attribute,
}

impl From<PyPayloadKind> for molgfx::PayloadKind {
    fn from(value: PyPayloadKind) -> Self {
        match value {
            PyPayloadKind::Structure => Self::Structure,
            PyPayloadKind::BondTopology => Self::BondTopology,
            PyPayloadKind::ScalarProperty => Self::ScalarProperty,
            PyPayloadKind::Trajectory => Self::Trajectory,
            PyPayloadKind::VolumeBrick => Self::VolumeBrick,
            PyPayloadKind::LabelBrick => Self::LabelBrick,
            PyPayloadKind::Mesh => Self::Mesh,
            PyPayloadKind::Proxy => Self::Proxy,
            PyPayloadKind::PointBatch => Self::PointBatch,
            PyPayloadKind::InstanceBatch => Self::InstanceBatch,
            PyPayloadKind::RelationBatch => Self::RelationBatch,
            PyPayloadKind::Attribute => Self::Attribute,
        }
    }
}

impl From<molgfx::PayloadKind> for PyPayloadKind {
    fn from(value: molgfx::PayloadKind) -> Self {
        match value {
            molgfx::PayloadKind::Structure => Self::Structure,
            molgfx::PayloadKind::BondTopology => Self::BondTopology,
            molgfx::PayloadKind::ScalarProperty => Self::ScalarProperty,
            molgfx::PayloadKind::Trajectory => Self::Trajectory,
            molgfx::PayloadKind::VolumeBrick => Self::VolumeBrick,
            molgfx::PayloadKind::LabelBrick => Self::LabelBrick,
            molgfx::PayloadKind::Mesh => Self::Mesh,
            molgfx::PayloadKind::Proxy => Self::Proxy,
            molgfx::PayloadKind::PointBatch => Self::PointBatch,
            molgfx::PayloadKind::InstanceBatch => Self::InstanceBatch,
            molgfx::PayloadKind::RelationBatch => Self::RelationBatch,
            molgfx::PayloadKind::Attribute => Self::Attribute,
        }
    }
}

#[pyclass(name = "ChunkSpan", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyChunkSpan(pub(crate) molgfx::ChunkSpan);

#[pymethods]
impl PyChunkSpan {
    #[new]
    fn new(first: PyLogicalRow, row_count: u32) -> PyResult<Self> {
        dataset(molgfx::ChunkSpan::new(first.0, row_count)).map(Self)
    }

    #[getter]
    fn first(&self) -> PyLogicalRow {
        PyLogicalRow(self.0.first())
    }

    #[getter]
    fn row_count(&self) -> u32 {
        self.0.row_count()
    }

    #[getter]
    fn end(&self) -> PyLogicalRow {
        PyLogicalRow(self.0.end())
    }

    fn local_row(&self, chunk: PyChunkId, row: PyLogicalRow) -> PyResult<PyLocalRow> {
        dataset(self.0.local_row(chunk.0, row.0)).map(PyLocalRow)
    }

    fn logical_row(&self, chunk: PyChunkId, row: PyLocalRow) -> PyResult<PyLogicalRow> {
        dataset(self.0.logical_row(chunk.0, row.0)).map(PyLogicalRow)
    }
}

#[pyclass(name = "ChunkBounds", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyChunkBounds(pub(crate) molgfx::ChunkBounds);

#[pymethods]
impl PyChunkBounds {
    #[new]
    fn new(min: [f32; 3], max: [f32; 3]) -> PyResult<Self> {
        dataset(molgfx::ChunkBounds::new(min, max)).map(Self)
    }

    #[getter]
    fn min(&self) -> [f32; 3] {
        self.0.min
    }

    #[getter]
    fn max(&self) -> [f32; 3] {
        self.0.max
    }
}

#[pyclass(name = "ChunkDescriptor", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyChunkDescriptor(pub(crate) molgfx::ChunkDescriptor);

#[pymethods]
impl PyChunkDescriptor {
    #[new]
    fn new(
        id: PyChunkId,
        parent: Option<PyChunkId>,
        level: u16,
        rows: PyChunkSpan,
        bounds: PyChunkBounds,
        payload_kind: PyPayloadKind,
        footprint: PyChunkFootprint,
    ) -> Self {
        Self(molgfx::ChunkDescriptor {
            id: id.0,
            parent: parent.map(|value| value.0),
            level,
            rows: rows.0,
            bounds: bounds.0,
            payload_kind: payload_kind.into(),
            footprint: footprint.0,
        })
    }

    #[getter]
    fn id(&self) -> PyChunkId {
        PyChunkId(self.0.id)
    }

    #[getter]
    fn parent(&self) -> Option<PyChunkId> {
        self.0.parent.map(PyChunkId)
    }

    #[getter]
    fn level(&self) -> u16 {
        self.0.level
    }

    #[getter]
    fn rows(&self) -> PyChunkSpan {
        PyChunkSpan(self.0.rows)
    }

    #[getter]
    fn bounds(&self) -> PyChunkBounds {
        PyChunkBounds(self.0.bounds)
    }

    #[getter]
    fn payload_kind(&self) -> PyPayloadKind {
        self.0.payload_kind.into()
    }

    #[getter]
    fn footprint(&self) -> PyChunkFootprint {
        PyChunkFootprint(self.0.footprint)
    }
}

#[pyclass(name = "DatasetCatalog", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyDatasetCatalog(pub(crate) molgfx::DatasetCatalog);

#[pymethods]
impl PyDatasetCatalog {
    #[staticmethod]
    fn from_descriptors(
        dataset_id: PyDatasetId,
        descriptors: Vec<PyChunkDescriptor>,
    ) -> PyResult<Self> {
        let descriptors = descriptors.into_iter().map(|value| value.0).collect();
        dataset(molgfx::DatasetCatalog::new(dataset_id.0, descriptors)).map(Self)
    }

    #[getter]
    fn dataset_id(&self) -> PyDatasetId {
        PyDatasetId(self.0.dataset_id())
    }

    #[getter]
    fn descriptor_count(&self) -> usize {
        self.0.len()
    }

    #[getter]
    fn ownership(&self) -> &'static str {
        "owned_metadata"
    }

    fn descriptor(&self, id: PyChunkId) -> Option<PyChunkDescriptor> {
        self.0.get(id.0).copied().map(PyChunkDescriptor)
    }

    fn copy_roots(&self) -> Vec<PyChunkDescriptor> {
        self.0.roots().copied().map(PyChunkDescriptor).collect()
    }

    fn copy_children(&self, parent: PyChunkId) -> Vec<PyChunkDescriptor> {
        self.0
            .children(parent.0)
            .copied()
            .map(PyChunkDescriptor)
            .collect()
    }

    fn total_footprint(&self) -> PyResult<PyChunkFootprint> {
        dataset(self.0.total_footprint()).map(PyChunkFootprint)
    }
}
