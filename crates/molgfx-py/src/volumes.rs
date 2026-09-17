//! Python adapters for volume data and representation materials.

use crate::error::core;
use crate::math::PyMat4;
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "ScalarVolume", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyScalarVolume(pub(crate) molgfx::ScalarVolume);

#[pymethods]
impl PyScalarVolume {
    #[new]
    #[pyo3(signature = (dimensions, values, voxel_to_world=None))]
    fn new(
        dimensions: (u32, u32, u32),
        values: Vec<f32>,
        voxel_to_world: Option<PyMat4>,
    ) -> PyResult<Self> {
        let transform = voxel_to_world.map_or(molgfx::Mat4::IDENTITY, |value| value.0);
        core(molgfx::ScalarVolume::new(
            [dimensions.0, dimensions.1, dimensions.2],
            transform,
            Arc::from(values.into_boxed_slice()),
        ))
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
    fn values(&self) -> Vec<f32> {
        self.0.values().to_vec()
    }
    #[getter]
    fn voxel_to_world(&self) -> PyMat4 {
        PyMat4(self.0.voxel_to_world())
    }
}

/// Compact declaration for a GPU-resident temporal occupancy volume.
#[pyclass(name = "OccupancyStream", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyOccupancyStream(pub(crate) molgfx::OccupancyStream);

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
        core(molgfx::OccupancyStream::new(
            [dimensions.0, dimensions.1, dimensions.2],
            molgfx::Vec3::new(origin.0, origin.1, origin.2),
            molgfx::Vec3::new(spacing.0, spacing.1, spacing.2),
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
pub(crate) struct PySegmentedVolume(pub(crate) molgfx::SegmentedVolume);

#[pymethods]
impl PySegmentedVolume {
    #[new]
    #[pyo3(signature = (dimensions, labels, voxel_to_world=None))]
    fn new(
        dimensions: (u32, u32, u32),
        labels: Vec<u32>,
        voxel_to_world: Option<PyMat4>,
    ) -> PyResult<Self> {
        let transform = voxel_to_world.map_or(molgfx::Mat4::IDENTITY, |value| value.0);
        core(molgfx::SegmentedVolume::new(
            [dimensions.0, dimensions.1, dimensions.2],
            transform,
            Arc::from(labels.into_boxed_slice()),
        ))
        .map(Self)
    }

    #[getter]
    fn dimensions(&self) -> (u32, u32, u32) {
        let value = self.0.dimensions();
        (value[0], value[1], value[2])
    }
    #[getter]
    fn labels(&self) -> Vec<u32> {
        self.0.labels().to_vec()
    }
    #[getter]
    fn voxel_to_world(&self) -> PyMat4 {
        PyMat4(self.0.voxel_to_world())
    }
}

#[pyclass(name = "MaterialModel", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyMaterialModel(pub(crate) molgfx::MaterialModel);

#[pymethods]
impl PyMaterialModel {
    #[staticmethod]
    fn molecular() -> Self {
        Self(molgfx::MaterialModel::Molecular)
    }
    #[staticmethod]
    fn principled(metallic: f32) -> Self {
        Self(molgfx::MaterialModel::Principled { metallic })
    }
    #[staticmethod]
    fn anisotropic_ribbon(strength: f32) -> Self {
        Self(molgfx::MaterialModel::AnisotropicRibbon { strength })
    }
    #[staticmethod]
    fn diffusion(strength: f32) -> Self {
        Self(molgfx::MaterialModel::Diffusion { strength })
    }
    fn __repr__(&self) -> String {
        match self.0 {
            molgfx::MaterialModel::Molecular => "MaterialModel.Molecular".to_owned(),
            molgfx::MaterialModel::Principled { metallic } => {
                format!("MaterialModel.Principled({metallic})")
            }
            molgfx::MaterialModel::AnisotropicRibbon { strength } => {
                format!("MaterialModel.AnisotropicRibbon({strength})")
            }
            molgfx::MaterialModel::Diffusion { strength } => {
                format!("MaterialModel.Diffusion({strength})")
            }
        }
    }
}

#[pyclass(name = "Material", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyMaterial(pub(crate) molgfx::Material);

#[pymethods]
impl PyMaterial {
    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::Material::default())
    }
    #[staticmethod]
    fn principled(metallic: f32) -> Self {
        Self(molgfx::Material::principled(metallic))
    }
    #[staticmethod]
    fn anisotropic_ribbon(strength: f32) -> Self {
        Self(molgfx::Material::anisotropic_ribbon(strength))
    }
    #[staticmethod]
    fn diffusion(strength: f32) -> Self {
        Self(molgfx::Material::diffusion(strength))
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyScalarVolume>()?;
    module.add_class::<PyOccupancyStream>()?;
    module.add_class::<PySegmentedVolume>()?;
    module.add_class::<PyMaterialModel>()?;
    module.add_class::<PyMaterial>()
}
