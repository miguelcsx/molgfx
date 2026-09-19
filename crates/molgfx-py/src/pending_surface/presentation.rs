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

impl From<PyFaceVisibility> for molgfx::core::FaceVisibility {
    fn from(value: PyFaceVisibility) -> Self {
        match value {
            PyFaceVisibility::DoubleSided => Self::DoubleSided,
            PyFaceVisibility::FrontOnly => Self::FrontOnly,
            PyFaceVisibility::BackOnly => Self::BackOnly,
        }
    }
}

impl From<molgfx::core::FaceVisibility> for PyFaceVisibility {
    fn from(value: molgfx::core::FaceVisibility) -> Self {
        match value {
            molgfx::core::FaceVisibility::DoubleSided => Self::DoubleSided,
            molgfx::core::FaceVisibility::FrontOnly => Self::FrontOnly,
            molgfx::core::FaceVisibility::BackOnly => Self::BackOnly,
        }
    }
}

#[pyclass(name = "SurfaceComponentPolicy", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PySurfaceComponentPolicy(pub(crate) molgfx::core::SurfaceComponentPolicy);

#[pymethods]
impl PySurfaceComponentPolicy {
    #[new]
    fn new() -> Self {
        Self(molgfx::core::SurfaceComponentPolicy::keep_all())
    }

    #[staticmethod]
    fn keep_all() -> Self {
        Self(molgfx::core::SurfaceComponentPolicy::keep_all())
    }

    #[staticmethod]
    fn minimum_area(area: f64) -> PyResult<Self> {
        molgfx::core::SurfaceComponentPolicy::minimum_area(area)
            .map(Self)
            .map_err(|error| crate::error::value(error.to_string()))
    }

    #[staticmethod]
    fn minimum_volume(volume: f64) -> PyResult<Self> {
        molgfx::core::SurfaceComponentPolicy::minimum_volume(volume)
            .map(Self)
            .map_err(|error| crate::error::value(error.to_string()))
    }

    #[staticmethod]
    fn minimum_voxels(voxels: u64) -> PyResult<Self> {
        molgfx::core::SurfaceComponentPolicy::minimum_voxels(voxels)
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

    /// Validated connected-component threshold.
    #[getter]
    fn threshold(&self) -> PySurfaceComponentThreshold {
        PySurfaceComponentThreshold(self.0.threshold())
    }

    fn is_enabled(&self) -> bool {
        self.0.is_enabled()
    }
}

/// Quantity a caller measures a sampled connected component by.
///
/// The variants that carry a measurement are constructed by name — either keep
/// every component, or set one minimum expressed as an area, a volume or a
/// voxel count.
#[pyclass(name = "SurfaceComponentThreshold", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PySurfaceComponentThreshold(pub(crate) molgfx::core::SurfaceComponentThreshold);

#[pymethods]
impl PySurfaceComponentThreshold {
    /// Keep every component.
    #[classattr]
    #[pyo3(name = "Disabled")]
    fn disabled_variant() -> Self {
        Self(molgfx::core::SurfaceComponentThreshold::Disabled)
    }

    /// Minimum exposed-face area in square ångström.
    #[staticmethod]
    fn area(square_angstrom: f64) -> Self {
        Self(molgfx::core::SurfaceComponentThreshold::Area(
            square_angstrom,
        ))
    }

    /// Minimum occupied volume in cubic ångström.
    #[staticmethod]
    fn volume(cubic_angstrom: f64) -> Self {
        Self(molgfx::core::SurfaceComponentThreshold::Volume(
            cubic_angstrom,
        ))
    }

    /// Minimum occupied voxel count.
    #[staticmethod]
    fn voxels(count: u64) -> Self {
        Self(molgfx::core::SurfaceComponentThreshold::Voxels(count))
    }

    /// `disabled`, `area`, `volume` or `voxels`.
    #[getter]
    fn measure(&self) -> String {
        match self.0 {
            molgfx::core::SurfaceComponentThreshold::Disabled => "disabled",
            molgfx::core::SurfaceComponentThreshold::Area(_) => "area",
            molgfx::core::SurfaceComponentThreshold::Volume(_) => "volume",
            molgfx::core::SurfaceComponentThreshold::Voxels(_) => "voxels",
        }
        .to_owned()
    }

    /// Minimum exposed-face area, when the threshold measures area.
    #[getter]
    fn minimum_area(&self) -> Option<f64> {
        match self.0 {
            molgfx::core::SurfaceComponentThreshold::Area(value) => Some(value),
            _ => None,
        }
    }

    /// Minimum occupied volume, when the threshold measures volume.
    #[getter]
    fn minimum_volume(&self) -> Option<f64> {
        match self.0 {
            molgfx::core::SurfaceComponentThreshold::Volume(value) => Some(value),
            _ => None,
        }
    }

    /// Minimum occupied voxel count, when the threshold counts voxels.
    #[getter]
    fn minimum_voxels(&self) -> Option<u64> {
        match self.0 {
            molgfx::core::SurfaceComponentThreshold::Voxels(value) => Some(value),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        format!("SurfaceComponentThreshold({})", self.measure())
    }
}

#[pyclass(name = "PropertyLegend", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyPropertyLegend(pub(crate) molgfx::core::PropertyLegend);

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
        Self(molgfx::core::PropertyLegend {
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
