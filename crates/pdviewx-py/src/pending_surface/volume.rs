//! Fixed-capacity volume-presentation adapters.

use crate::error::core;
use crate::math::PyRgba8;
use pyo3::prelude::*;

#[pyclass(name = "VolumeRendering", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyVolumeRendering {
    Direct,
    Isosurface,
    Medium,
    Slice,
    LiquidSurface,
}

impl From<PyVolumeRendering> for pdviewx::VolumeRendering {
    fn from(value: PyVolumeRendering) -> Self {
        match value {
            PyVolumeRendering::Direct => Self::Direct,
            PyVolumeRendering::Isosurface => Self::Isosurface,
            PyVolumeRendering::Medium => Self::Medium,
            PyVolumeRendering::Slice => Self::Slice,
            PyVolumeRendering::LiquidSurface => Self::LiquidSurface,
        }
    }
}

impl From<pdviewx::VolumeRendering> for PyVolumeRendering {
    fn from(value: pdviewx::VolumeRendering) -> Self {
        match value {
            pdviewx::VolumeRendering::Direct => Self::Direct,
            pdviewx::VolumeRendering::Isosurface => Self::Isosurface,
            pdviewx::VolumeRendering::Medium => Self::Medium,
            pdviewx::VolumeRendering::Slice => Self::Slice,
            pdviewx::VolumeRendering::LiquidSurface => Self::LiquidSurface,
        }
    }
}

#[pyclass(name = "VolumeTransferPoint", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyVolumeTransferPoint(pub(crate) pdviewx::VolumeTransferPoint);

#[pymethods]
impl PyVolumeTransferPoint {
    #[new]
    fn new(value: f32, color: PyRgba8, opacity: f32) -> Self {
        Self(pdviewx::VolumeTransferPoint::new(value, color.0, opacity))
    }

    #[getter]
    fn value(&self) -> f32 {
        self.0.value
    }

    #[getter]
    fn color(&self) -> PyRgba8 {
        self.0.color.into()
    }

    #[getter]
    fn opacity(&self) -> f32 {
        self.0.opacity
    }
}

#[pyclass(name = "VolumeTransferFunction", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyVolumeTransferFunction(pub(crate) pdviewx::VolumeTransferFunction);

#[pymethods]
impl PyVolumeTransferFunction {
    #[new]
    fn new(points: Vec<PyVolumeTransferPoint>) -> PyResult<Self> {
        let points = points.into_iter().map(|point| point.0).collect::<Vec<_>>();
        core(pdviewx::VolumeTransferFunction::new(&points)).map(Self)
    }

    #[staticmethod]
    fn linear(range: [f32; 2], low: PyRgba8, high: PyRgba8) -> Self {
        Self(pdviewx::VolumeTransferFunction::linear(
            range, low.0, high.0,
        ))
    }

    #[staticmethod]
    fn default() -> Self {
        Self(pdviewx::VolumeTransferFunction::default())
    }

    fn copy_points(&self) -> Vec<PyVolumeTransferPoint> {
        self.0
            .points()
            .iter()
            .copied()
            .map(PyVolumeTransferPoint)
            .collect()
    }
}
