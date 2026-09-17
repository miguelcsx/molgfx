//! Mesh and property-presentation value adapters.

use crate::math::PyRgba8;
use crate::values::PyScalarFieldSemantics;
use pyo3::prelude::*;

#[pyclass(name = "FaceVisibility", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyFaceVisibility {
    DoubleSided,
    FrontOnly,
    BackOnly,
}

impl From<PyFaceVisibility> for molgfx::FaceVisibility {
    fn from(value: PyFaceVisibility) -> Self {
        match value {
            PyFaceVisibility::DoubleSided => Self::DoubleSided,
            PyFaceVisibility::FrontOnly => Self::FrontOnly,
            PyFaceVisibility::BackOnly => Self::BackOnly,
        }
    }
}

impl From<molgfx::FaceVisibility> for PyFaceVisibility {
    fn from(value: molgfx::FaceVisibility) -> Self {
        match value {
            molgfx::FaceVisibility::DoubleSided => Self::DoubleSided,
            molgfx::FaceVisibility::FrontOnly => Self::FrontOnly,
            molgfx::FaceVisibility::BackOnly => Self::BackOnly,
        }
    }
}

#[pyclass(name = "SurfaceComponentPolicy", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PySurfaceComponentPolicy(pub(crate) molgfx::SurfaceComponentPolicy);

#[pymethods]
impl PySurfaceComponentPolicy {
    #[new]
    fn new() -> Self {
        Self(molgfx::SurfaceComponentPolicy::keep_all())
    }

    #[staticmethod]
    fn keep_all() -> Self {
        Self(molgfx::SurfaceComponentPolicy::keep_all())
    }

    #[staticmethod]
    fn minimum_area(area: f64) -> PyResult<Self> {
        molgfx::SurfaceComponentPolicy::minimum_area(area)
            .map(Self)
            .map_err(|error| crate::error::value(error.to_string()))
    }

    #[staticmethod]
    fn minimum_volume(volume: f64) -> PyResult<Self> {
        molgfx::SurfaceComponentPolicy::minimum_volume(volume)
            .map(Self)
            .map_err(|error| crate::error::value(error.to_string()))
    }

    #[staticmethod]
    fn minimum_voxels(voxels: u64) -> PyResult<Self> {
        molgfx::SurfaceComponentPolicy::minimum_voxels(voxels)
            .map(Self)
            .map_err(|error| crate::error::value(error.to_string()))
    }

    fn with_maximum_components(&self, count: u32) -> PyResult<Self> {
        self.0
            .with_maximum_components(count)
            .map(Self)
            .map_err(|error| crate::error::value(error.to_string()))
    }

    #[getter]
    fn maximum_components(&self) -> Option<u32> {
        self.0.maximum_components()
    }

    fn is_enabled(&self) -> bool {
        self.0.is_enabled()
    }
}

#[pyclass(name = "PropertyLegend", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyPropertyLegend(pub(crate) molgfx::PropertyLegend);

#[pymethods]
impl PyPropertyLegend {
    #[new]
    fn new(
        title: String,
        semantics: PyScalarFieldSemantics,
        values: [f32; 3],
        colors: [PyRgba8; 3],
        missing: PyRgba8,
    ) -> Self {
        Self(molgfx::PropertyLegend {
            title: title.into(),
            semantics: semantics.0,
            values,
            colors: colors.map(|color| color.0),
            missing: missing.0,
        })
    }

    #[getter]
    fn title(&self) -> &str {
        &self.0.title
    }

    #[getter]
    fn semantics(&self) -> PyScalarFieldSemantics {
        PyScalarFieldSemantics(self.0.semantics.clone())
    }

    #[getter]
    fn values(&self) -> [f32; 3] {
        self.0.values
    }

    #[getter]
    fn colors(&self) -> [PyRgba8; 3] {
        self.0.colors.map(PyRgba8)
    }

    #[getter]
    fn missing(&self) -> PyRgba8 {
        PyRgba8(self.0.missing)
    }
}
