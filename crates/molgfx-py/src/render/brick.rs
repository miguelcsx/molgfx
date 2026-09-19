//! Mechanical Python values for sparse brick metadata owned and validated by Rust.

use crate::error::dataset;
use crate::semantic::{PyChunkId, PyDatasetId};
use pyo3::prelude::*;

#[pyclass(name = "BrickId", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PyBrickId(pub(crate) molgfx::core::BrickId);

#[pymethods]
impl PyBrickId {
    #[new]
    fn new(value: u64) -> Self {
        Self(molgfx::core::BrickId::new(value))
    }

    #[getter]
    fn value(&self) -> u64 {
        self.0.get()
    }

    fn __int__(&self) -> u64 {
        self.0.get()
    }
}

#[pyclass(name = "BrickAddress", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyBrickAddress(pub(crate) molgfx::core::BrickAddress);

#[pymethods]
impl PyBrickAddress {
    #[new]
    fn new(origin: [u64; 3], mip: u16) -> Self {
        Self(molgfx::core::BrickAddress { origin, mip })
    }

    #[getter]
    fn origin(&self) -> [u64; 3] {
        self.0.origin
    }

    #[getter]
    fn mip(&self) -> u16 {
        self.0.mip
    }
}

#[pyclass(name = "DirtyGeneration", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PyDirtyGeneration(pub(crate) molgfx::core::DirtyGeneration);

#[pymethods]
impl PyDirtyGeneration {
    #[new]
    fn new(value: u64) -> Self {
        Self(molgfx::core::DirtyGeneration::new(value))
    }

    #[getter]
    fn value(&self) -> u64 {
        self.0.get()
    }

    fn next(&self) -> PyResult<Self> {
        dataset(self.0.next()).map(Self)
    }
}

#[pyclass(name = "BrickValueRange", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyBrickValueRange(pub(crate) molgfx::core::BrickValueRange);

#[pymethods]
impl PyBrickValueRange {
    #[staticmethod]
    fn scalar(min: f32, max: f32) -> Self {
        Self(molgfx::core::BrickValueRange::Scalar { min, max })
    }

    #[staticmethod]
    fn segmentation(min: u32, max: u32) -> Self {
        Self(molgfx::core::BrickValueRange::Segmentation { min, max })
    }

    #[staticmethod]
    fn occupancy(has_empty: bool, has_occupied: bool) -> Self {
        Self(molgfx::core::BrickValueRange::Occupancy {
            has_empty,
            has_occupied,
        })
    }

    #[getter]
    fn kind(&self) -> &'static str {
        match self.0 {
            molgfx::core::BrickValueRange::Scalar { .. } => "scalar",
            molgfx::core::BrickValueRange::Segmentation { .. } => "segmentation",
            molgfx::core::BrickValueRange::Occupancy { .. } => "occupancy",
        }
    }
}

#[pyclass(name = "BrickShape", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyBrickShape(pub(crate) molgfx::core::BrickShape);

#[pymethods]
impl PyBrickShape {
    #[new]
    fn new(stored: [u16; 3], halo: u16) -> PyResult<Self> {
        dataset(molgfx::core::BrickShape::new(stored, halo)).map(Self)
    }

    #[getter]
    fn stored(&self) -> [u16; 3] {
        self.0.stored()
    }

    #[getter]
    fn interior(&self) -> [u16; 3] {
        self.0.interior()
    }

    #[getter]
    fn halo(&self) -> u16 {
        self.0.halo()
    }

    #[getter]
    fn voxel_count(&self) -> u32 {
        self.0.voxel_count()
    }
}

#[pyclass(name = "BrickMetadata", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyBrickMetadata(pub(crate) molgfx::core::BrickMetadata);

#[pymethods]
impl PyBrickMetadata {
    #[new]
    fn new(
        id: PyBrickId,
        address: PyBrickAddress,
        shape: PyBrickShape,
        value_range: PyBrickValueRange,
        generation: PyDirtyGeneration,
    ) -> PyResult<Self> {
        dataset(molgfx::core::BrickMetadata::new(
            id.0,
            address.0,
            shape.0,
            value_range.0,
            generation.0,
        ))
        .map(Self)
    }

    #[getter]
    fn id(&self) -> PyBrickId {
        PyBrickId(self.0.id)
    }

    #[getter]
    fn address(&self) -> PyBrickAddress {
        PyBrickAddress(self.0.address)
    }

    #[getter]
    fn shape(&self) -> PyBrickShape {
        PyBrickShape(self.0.shape)
    }

    #[getter]
    fn value_range(&self) -> PyBrickValueRange {
        PyBrickValueRange(self.0.range)
    }

    #[getter]
    fn generation(&self) -> PyDirtyGeneration {
        PyDirtyGeneration(self.0.generation)
    }
}

#[pyclass(name = "BrickDescriptor", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyBrickDescriptor(pub(crate) molgfx::core::BrickDescriptor);

#[pymethods]
impl PyBrickDescriptor {
    #[new]
    fn new(chunk: PyChunkId, metadata: PyBrickMetadata) -> Self {
        Self(molgfx::core::BrickDescriptor {
            chunk: chunk.0,
            metadata: metadata.0,
        })
    }

    #[getter]
    fn chunk(&self) -> PyChunkId {
        PyChunkId(self.0.chunk)
    }

    #[getter]
    fn metadata(&self) -> PyBrickMetadata {
        PyBrickMetadata(self.0.metadata)
    }
}

#[pyclass(name = "BrickCatalog", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyBrickCatalog(pub(crate) molgfx::core::BrickCatalog);

#[pymethods]
impl PyBrickCatalog {
    #[staticmethod]
    fn from_descriptors(
        dataset_id: PyDatasetId,
        logical_extent: [u64; 3],
        voxel_bytes: u16,
        descriptors: Vec<PyBrickDescriptor>,
    ) -> PyResult<Self> {
        let descriptors = descriptors.into_iter().map(|value| value.0).collect();
        dataset(molgfx::core::BrickCatalog::new(
            dataset_id.0,
            logical_extent,
            voxel_bytes,
            descriptors,
        ))
        .map(Self)
    }

    #[getter]
    fn dataset_id(&self) -> PyDatasetId {
        PyDatasetId(self.0.dataset_id())
    }

    #[getter]
    fn logical_extent(&self) -> [u64; 3] {
        self.0.logical_extent()
    }

    #[getter]
    fn descriptor_count(&self) -> usize {
        self.0.descriptors().len()
    }

    fn copy_descriptors(&self) -> Vec<PyBrickDescriptor> {
        self.0
            .descriptors()
            .iter()
            .copied()
            .map(PyBrickDescriptor)
            .collect()
    }

    fn logical_bytes(&self) -> PyResult<u64> {
        dataset(self.0.logical_bytes())
    }
}
