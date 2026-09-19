//! Python adapters for volume data and representation materials.

use crate::error::core;
use crate::math::PyMat4;
use numpy::{PyArray1, PyReadonlyArray1};
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "ScalarVolume", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyScalarVolume(pub(crate) molgfx::core::ScalarVolume);

#[pymethods]
impl PyScalarVolume {
    #[new]
    #[pyo3(signature = (dimensions, values, voxel_to_world=None))]
    fn new(
        py: Python<'_>,
        dimensions: (u32, u32, u32),
        values: PyReadonlyArray1<'_, f32>,
        voxel_to_world: Option<PyMat4>,
    ) -> PyResult<Self> {
        let values = values
            .as_slice()
            .map_err(|_| crate::error::value("values must be C-contiguous float32"))?;
        let transform = voxel_to_world.map_or(molgfx::math::Mat4::IDENTITY, |value| value.0);
        core(py.detach(|| {
            molgfx::core::ScalarVolume::new(
                [dimensions.0, dimensions.1, dimensions.2],
                transform,
                Arc::from(values),
            )
        }))
        .map(Self)
    }

    #[getter]
    fn dimensions(&self) -> (u32, u32, u32) {
        let value = self.0.dimensions();
        (value[0], value[1], value[2])
    }
    #[getter]
    fn range(&self) -> (f32, f32) {
        let value = self.0.range();
        (value[0], value[1])
    }
    #[getter]
    fn values<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f32>> {
        PyArray1::from_slice(py, self.0.values())
    }
    #[getter]
    fn voxel_to_world(&self) -> PyMat4 {
        PyMat4(self.0.voxel_to_world())
    }
}

/// Compact declaration for a GPU-resident temporal occupancy volume.
#[pyclass(name = "OccupancyStream", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyOccupancyStream(pub(crate) molgfx::core::OccupancyStream);

#[pymethods]
impl PyOccupancyStream {
    #[new]
    #[pyo3(signature = (dimensions, origin, spacing, decay, deposit, maximum))]
    fn new(
        dimensions: (u32, u32, u32),
        origin: (f32, f32, f32),
        spacing: (f32, f32, f32),
        decay: f32,
        deposit: f32,
        maximum: f32,
    ) -> PyResult<Self> {
        core(molgfx::core::OccupancyStream::new(
            [dimensions.0, dimensions.1, dimensions.2],
            molgfx::math::Vec3::new(origin.0, origin.1, origin.2),
            molgfx::math::Vec3::new(spacing.0, spacing.1, spacing.2),
            decay,
            deposit,
            maximum,
        ))
        .map(Self)
    }

    #[getter]
    fn dimensions(&self) -> (u32, u32, u32) {
        let value = self.0.dimensions();
        (value[0], value[1], value[2])
    }
    #[getter]
    fn voxel_to_model(&self) -> PyMat4 {
        PyMat4(self.0.voxel_to_model())
    }
    #[getter]
    fn decay(&self) -> f32 {
        self.0.decay()
    }
    #[getter]
    fn deposit(&self) -> f32 {
        self.0.deposit()
    }
    #[getter]
    fn maximum(&self) -> f32 {
        self.0.maximum()
    }
}

#[pyclass(name = "SegmentedVolume", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PySegmentedVolume(pub(crate) molgfx::core::SegmentedVolume);

#[pymethods]
impl PySegmentedVolume {
    #[new]
    #[pyo3(signature = (dimensions, labels, voxel_to_world=None))]
    fn new(
        py: Python<'_>,
        dimensions: (u32, u32, u32),
        labels: PyReadonlyArray1<'_, u32>,
        voxel_to_world: Option<PyMat4>,
    ) -> PyResult<Self> {
        let labels = labels
            .as_slice()
            .map_err(|_| crate::error::value("labels must be C-contiguous uint32"))?;
        let transform = voxel_to_world.map_or(molgfx::math::Mat4::IDENTITY, |value| value.0);
        core(py.detach(|| {
            molgfx::core::SegmentedVolume::new(
                [dimensions.0, dimensions.1, dimensions.2],
                transform,
                Arc::from(labels),
            )
        }))
        .map(Self)
    }

    #[getter]
    fn dimensions(&self) -> (u32, u32, u32) {
        let value = self.0.dimensions();
        (value[0], value[1], value[2])
    }
    #[getter]
    fn labels<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<u32>> {
        PyArray1::from_slice(py, self.0.labels())
    }
    #[getter]
    fn voxel_to_world(&self) -> PyMat4 {
        PyMat4(self.0.voxel_to_world())
    }
}

#[pyclass(name = "MaterialModel", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyMaterialModel(pub(crate) molgfx::core::MaterialModel);

#[pymethods]
impl PyMaterialModel {
    #[staticmethod]
    fn molecular() -> Self {
        Self(molgfx::core::MaterialModel::Molecular)
    }
    #[staticmethod]
    fn principled(metallic: f32) -> Self {
        Self(molgfx::core::MaterialModel::Principled { metallic })
    }
    #[staticmethod]
    fn anisotropic_ribbon(strength: f32) -> Self {
        Self(molgfx::core::MaterialModel::AnisotropicRibbon { strength })
    }
    #[staticmethod]
    fn diffusion(strength: f32) -> Self {
        Self(molgfx::core::MaterialModel::Diffusion { strength })
    }
    fn __repr__(&self) -> String {
        match self.0 {
            molgfx::core::MaterialModel::Molecular => "MaterialModel.Molecular".to_owned(),
            molgfx::core::MaterialModel::Principled { metallic } => {
                format!("MaterialModel.Principled({metallic})")
            }
            molgfx::core::MaterialModel::AnisotropicRibbon { strength } => {
                format!("MaterialModel.AnisotropicRibbon({strength})")
            }
            molgfx::core::MaterialModel::Diffusion { strength } => {
                format!("MaterialModel.Diffusion({strength})")
            }
        }
    }
}

#[pyclass(name = "Material", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyMaterial(pub(crate) molgfx::core::Material);

#[pymethods]
impl PyMaterial {
    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::core::Material::default())
    }
    #[staticmethod]
    fn principled(metallic: f32) -> Self {
        Self(molgfx::core::Material::principled(metallic))
    }
    #[staticmethod]
    fn anisotropic_ribbon(strength: f32) -> Self {
        Self(molgfx::core::Material::anisotropic_ribbon(strength))
    }
    #[staticmethod]
    fn diffusion(strength: f32) -> Self {
        Self(molgfx::core::Material::diffusion(strength))
    }
    #[getter]
    fn opacity(&self) -> f32 {
        self.0.opacity
    }
    #[getter]
    fn roughness(&self) -> f32 {
        self.0.roughness
    }
    #[getter]
    fn specular(&self) -> f32 {
        self.0.specular
    }
    #[getter]
    fn model(&self) -> PyMaterialModel {
        PyMaterialModel(self.0.model)
    }
}
