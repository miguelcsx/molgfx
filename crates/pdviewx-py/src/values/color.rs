//! Python adapters for colors, scalar fields and surface overlays.

use crate::core::PyVolumeHandle;
use crate::error::core;
use crate::math::PyRgba8;
use pyo3::prelude::*;

#[pyclass(name = "ColorScheme", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyColorScheme(pub(crate) pdviewx::ColorScheme);

#[pymethods]
impl PyColorScheme {
    #[staticmethod]
    fn by_element() -> Self {
        Self(pdviewx::ColorScheme::ByElement)
    }
    #[staticmethod]
    fn by_chain() -> Self {
        Self(pdviewx::ColorScheme::ByChain)
    }
    #[staticmethod]
    fn by_residue() -> Self {
        Self(pdviewx::ColorScheme::ByResidue)
    }
    #[staticmethod]
    fn by_secondary_structure() -> Self {
        Self(pdviewx::ColorScheme::BySecondaryStructure)
    }
    #[staticmethod]
    fn uniform(color: PyRgba8) -> Self {
        Self(pdviewx::ColorScheme::Uniform(color.0))
    }
    #[getter]
    fn kind(&self) -> &'static str {
        match self.0 {
            pdviewx::ColorScheme::ByElement => "by_element",
            pdviewx::ColorScheme::ByChain => "by_chain",
            pdviewx::ColorScheme::ByResidue => "by_residue",
            pdviewx::ColorScheme::BySecondaryStructure => "by_secondary_structure",
            pdviewx::ColorScheme::ByProperty { .. } => "by_property",
            pdviewx::ColorScheme::Uniform(_) => "uniform",
            _ => "unknown",
        }
    }
}

#[pyclass(name = "SurfaceKind", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PySurfaceKind {
    VanDerWaals,
    SolventAccessible,
    SolventExcluded,
    Gaussian,
}

impl From<PySurfaceKind> for pdviewx::SurfaceKind {
    fn from(value: PySurfaceKind) -> Self {
        match value {
            PySurfaceKind::VanDerWaals => Self::VanDerWaals,
            PySurfaceKind::SolventAccessible => Self::SolventAccessible,
            PySurfaceKind::SolventExcluded => Self::SolventExcluded,
            PySurfaceKind::Gaussian => Self::Gaussian,
        }
    }
}

#[pyclass(name = "SurfaceStyle", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PySurfaceStyle {
    Solid,
    Contour,
    Dots,
    FilledContour,
    Mesh,
    SoftUnion,
}

impl From<PySurfaceStyle> for pdviewx::SurfaceStyle {
    fn from(value: PySurfaceStyle) -> Self {
        match value {
            PySurfaceStyle::Solid => Self::Solid,
            PySurfaceStyle::Contour => Self::Contour,
            PySurfaceStyle::Dots => Self::Dots,
            PySurfaceStyle::FilledContour => Self::FilledContour,
            PySurfaceStyle::Mesh => Self::Mesh,
            PySurfaceStyle::SoftUnion => Self::SoftUnion,
        }
    }
}

#[pyclass(name = "ScalarFieldSemantics", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyScalarFieldSemantics(pub(crate) pdviewx::ScalarFieldSemantics);

#[pymethods]
impl PyScalarFieldSemantics {
    #[staticmethod]
    fn uncalibrated_rank() -> Self {
        Self(pdviewx::ScalarFieldSemantics::UncalibratedRank)
    }
    #[staticmethod]
    fn quantity(name: &str, units: &str, provenance: &str) -> PyResult<Self> {
        core(pdviewx::ScalarFieldSemantics::quantity(
            name.into(),
            units.into(),
            provenance.into(),
        ))
        .map(Self)
    }
    #[getter]
    fn kind(&self) -> &'static str {
        match self.0 {
            pdviewx::ScalarFieldSemantics::UncalibratedRank => "uncalibrated_rank",
            pdviewx::ScalarFieldSemantics::Quantity { .. } => "quantity",
        }
    }
}

#[pyclass(name = "ScalarRamp", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyScalarRamp(pub(crate) pdviewx::ScalarRamp);

impl From<pdviewx::ScalarRamp> for PyScalarRamp {
    fn from(value: pdviewx::ScalarRamp) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyScalarRamp {
    #[new]
    fn new(values: (f32, f32, f32), colors: (PyRgba8, PyRgba8, PyRgba8)) -> PyResult<Self> {
        core(pdviewx::ScalarRamp::new(
            [values.0, values.1, values.2],
            [colors.0.0, colors.1.0, colors.2.0],
        ))
        .map(Self)
    }
    #[staticmethod]
    fn diverging(extent: f32) -> Self {
        Self(pdviewx::ScalarRamp::diverging(extent))
    }
    #[staticmethod]
    fn sequential(domain: (f32, f32)) -> Self {
        Self(pdviewx::ScalarRamp::sequential([domain.0, domain.1]))
    }
    #[getter]
    fn values(&self) -> (f32, f32, f32) {
        let values = self.0.values();
        (values[0], values[1], values[2])
    }
    #[getter]
    fn colors(&self) -> (PyRgba8, PyRgba8, PyRgba8) {
        let colors = self.0.colors();
        (colors[0].into(), colors[1].into(), colors[2].into())
    }
    fn sample(&self, value: f32, missing: PyRgba8) -> PyRgba8 {
        self.0.sample(value, missing.0).into()
    }
}

#[pyclass(name = "ScalarContours", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyScalarContours(pub(crate) pdviewx::ScalarContours);

#[pymethods]
impl PyScalarContours {
    #[new]
    fn new(interval: f32, width_pixels: f32) -> PyResult<Self> {
        core(pdviewx::ScalarContours::new(interval, width_pixels)).map(Self)
    }
    #[getter]
    fn interval(&self) -> f32 {
        self.0.interval
    }
    #[getter]
    fn width_pixels(&self) -> f32 {
        self.0.width_pixels
    }
}

#[pyclass(name = "SurfaceScalarOverlay", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PySurfaceScalarOverlay(pub(crate) pdviewx::SurfaceScalarOverlay);

#[pymethods]
impl PySurfaceScalarOverlay {
    #[new]
    fn new(field: PyVolumeHandle, ramp: PyScalarRamp) -> Self {
        Self(pdviewx::SurfaceScalarOverlay::new(field.0, ramp.0))
    }
    #[getter]
    fn field(&self) -> PyVolumeHandle {
        self.0.field.into()
    }
    #[getter]
    fn ramp(&self) -> PyScalarRamp {
        self.0.ramp.into()
    }
    fn with_contours(&self, contours: PyScalarContours) -> Self {
        Self(pdviewx::SurfaceScalarOverlay {
            contours: Some(contours.0),
            ..self.0
        })
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyColorScheme>()?;
    module.add_class::<PySurfaceKind>()?;
    module.add_class::<PySurfaceStyle>()?;
    module.add_class::<PyScalarFieldSemantics>()?;
    module.add_class::<PyScalarRamp>()?;
    module.add_class::<PyScalarContours>()?;
    module.add_class::<PySurfaceScalarOverlay>()
}
