//! Python adapters for persistent annotations and caller-computed measurements.

use crate::core::{
    PyAnnotationHandle, PyEntityRef, PyMeasurementHandle, PyScene, PySelectionHandle,
    PyStructureHandle,
};
use crate::error::{core, value};
use crate::math::{PyRgba8, PyVec3};
use pyo3::prelude::*;

#[pyclass(name = "AnnotationAnchor", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyAnnotationAnchor(pub(crate) pdviewx::AnnotationAnchor);

#[pymethods]
impl PyAnnotationAnchor {
    #[staticmethod]
    fn world(position: PyVec3) -> PyResult<Self> {
        core(pdviewx::AnnotationAnchor::world(position.0)).map(Self)
    }

    #[staticmethod]
    fn entity(position: PyVec3, entity: PyEntityRef) -> PyResult<Self> {
        core(pdviewx::AnnotationAnchor::entity(position.0, entity.0)).map(Self)
    }

    #[getter]
    fn position(&self) -> PyVec3 {
        PyVec3(self.0.position())
    }

    #[getter]
    fn source_entity(&self) -> Option<PyEntityRef> {
        self.0.source_entity().map(PyEntityRef)
    }
}

#[pyclass(name = "MarkerShape", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyMarkerShape {
    Circle,
    Diamond,
    Crosshair,
}

impl From<PyMarkerShape> for pdviewx::MarkerShape {
    fn from(value: PyMarkerShape) -> Self {
        match value {
            PyMarkerShape::Circle => Self::Circle,
            PyMarkerShape::Diamond => Self::Diamond,
            PyMarkerShape::Crosshair => Self::Crosshair,
        }
    }
}

impl From<pdviewx::MarkerShape> for PyMarkerShape {
    fn from(value: pdviewx::MarkerShape) -> Self {
        match value {
            pdviewx::MarkerShape::Circle => Self::Circle,
            pdviewx::MarkerShape::Diamond => Self::Diamond,
            pdviewx::MarkerShape::Crosshair => Self::Crosshair,
        }
    }
}

#[pyclass(name = "MarkerStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyMarkerStyle(pub(crate) pdviewx::MarkerStyle);

#[pymethods]
impl PyMarkerStyle {
    #[new]
    #[pyo3(signature = (color=None, radius_pixels=6.0, shape=None))]
    fn new(color: Option<PyRgba8>, radius_pixels: f32, shape: Option<PyMarkerShape>) -> Self {
        let default = pdviewx::MarkerStyle::default();
        Self(pdviewx::MarkerStyle {
            color: color.map_or(default.color, |value| value.0),
            radius_pixels,
            shape: shape.map_or(default.shape, Into::into),
        })
    }

    #[getter]
    fn color(&self) -> PyRgba8 {
        self.0.color.into()
    }

    #[getter]
    fn radius_pixels(&self) -> f32 {
        self.0.radius_pixels
    }

    #[getter]
    fn shape(&self) -> PyMarkerShape {
        self.0.shape.into()
    }
}

#[pyclass(name = "AnnotationKind", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyAnnotationKind {
    Note,
    Marker,
    Region,
    Hypothesis,
}

impl From<pdviewx::AnnotationKind> for PyAnnotationKind {
    fn from(value: pdviewx::AnnotationKind) -> Self {
        match value {
            pdviewx::AnnotationKind::Note => Self::Note,
            pdviewx::AnnotationKind::Marker => Self::Marker,
            pdviewx::AnnotationKind::Region => Self::Region,
            pdviewx::AnnotationKind::Hypothesis => Self::Hypothesis,
        }
    }
}

#[pyclass(name = "Annotation", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyAnnotation(pub(crate) pdviewx::Annotation);

#[pymethods]
impl PyAnnotation {
    #[staticmethod]
    fn note(owner: PyStructureHandle, anchor: PyAnnotationAnchor, text: &str) -> PyResult<Self> {
        core(pdviewx::Annotation::note(owner.0, anchor.0, text)).map(Self)
    }

    #[staticmethod]
    fn hypothesis(
        owner: PyStructureHandle,
        anchor: PyAnnotationAnchor,
        text: &str,
    ) -> PyResult<Self> {
        core(pdviewx::Annotation::hypothesis(owner.0, anchor.0, text)).map(Self)
    }

    #[staticmethod]
    fn marker(
        owner: PyStructureHandle,
        anchor: PyAnnotationAnchor,
        style: PyMarkerStyle,
    ) -> PyResult<Self> {
        core(pdviewx::Annotation::marker(owner.0, anchor.0, style.0)).map(Self)
    }

    #[staticmethod]
    fn region(
        owner: PyStructureHandle,
        selection: PySelectionHandle,
        label: &str,
    ) -> PyResult<Self> {
        core(pdviewx::Annotation::region(owner.0, selection.0, label)).map(Self)
    }

    fn with_priority(&self, priority: i16) -> Self {
        Self(self.0.clone().with_priority(priority))
    }

    #[getter]
    fn owner(&self) -> PyStructureHandle {
        self.0.owner().into()
    }

    #[getter]
    fn kind(&self) -> PyAnnotationKind {
        self.0.kind().into()
    }

    #[getter]
    fn anchor(&self) -> Option<PyAnnotationAnchor> {
        self.0.anchor().map(PyAnnotationAnchor)
    }

    #[getter]
    fn region_selection(&self) -> Option<PySelectionHandle> {
        self.0.region_selection().map(Into::into)
    }

    #[getter]
    fn text(&self) -> String {
        self.0.text().to_owned()
    }

    #[getter]
    fn marker_style(&self) -> PyMarkerStyle {
        PyMarkerStyle(self.0.marker_style())
    }

    #[getter]
    fn priority(&self) -> i16 {
        self.0.priority()
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.is_visible()
    }
}

#[pyclass(name = "MeasurementKind", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyMeasurementKind {
    Distance,
    Angle,
    Dihedral,
}

impl From<pdviewx::MeasurementKind> for PyMeasurementKind {
    fn from(value: pdviewx::MeasurementKind) -> Self {
        match value {
            pdviewx::MeasurementKind::Distance => Self::Distance,
            pdviewx::MeasurementKind::Angle => Self::Angle,
            pdviewx::MeasurementKind::Dihedral => Self::Dihedral,
        }
    }
}

#[pyclass(name = "Measurement", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyMeasurement(pub(crate) pdviewx::Measurement);

fn fixed_anchors<const N: usize>(
    values: Vec<PyAnnotationAnchor>,
) -> PyResult<[pdviewx::AnnotationAnchor; N]> {
    let count = values.len();
    values
        .try_into()
        .map(|values: [PyAnnotationAnchor; N]| values.map(|anchor| anchor.0))
        .map_err(|_| value(format!("expected {N} anchors, got {count}")))
}

#[pymethods]
impl PyMeasurement {
    #[staticmethod]
    fn distance(
        owner: PyStructureHandle,
        anchors: Vec<PyAnnotationAnchor>,
        value: f32,
        provenance: &str,
    ) -> PyResult<Self> {
        core(pdviewx::Measurement::distance(
            owner.0,
            fixed_anchors::<2>(anchors)?,
            value,
            provenance,
        ))
        .map(Self)
    }

    #[staticmethod]
    fn angle(
        owner: PyStructureHandle,
        anchors: Vec<PyAnnotationAnchor>,
        value: f32,
        provenance: &str,
    ) -> PyResult<Self> {
        core(pdviewx::Measurement::angle(
            owner.0,
            fixed_anchors::<3>(anchors)?,
            value,
            provenance,
        ))
        .map(Self)
    }

    #[staticmethod]
    fn dihedral(
        owner: PyStructureHandle,
        anchors: Vec<PyAnnotationAnchor>,
        value: f32,
        provenance: &str,
    ) -> PyResult<Self> {
        core(pdviewx::Measurement::dihedral(
            owner.0,
            fixed_anchors::<4>(anchors)?,
            value,
            provenance,
        ))
        .map(Self)
    }

    fn with_priority(&self, priority: i16) -> Self {
        Self(self.0.clone().with_priority(priority))
    }

    #[getter]
    fn owner(&self) -> PyStructureHandle {
        self.0.owner().into()
    }

    #[getter]
    fn kind(&self) -> PyMeasurementKind {
        self.0.kind().into()
    }

    #[getter]
    fn anchors(&self) -> Vec<PyAnnotationAnchor> {
        self.0
            .anchors()
            .iter()
            .copied()
            .map(PyAnnotationAnchor)
            .collect()
    }

    #[getter]
    fn value(&self) -> f32 {
        self.0.value()
    }

    #[getter]
    fn label(&self) -> String {
        self.0.label().to_owned()
    }

    #[getter]
    fn provenance(&self) -> String {
        self.0.provenance().to_owned()
    }

    #[getter]
    fn priority(&self) -> i16 {
        self.0.priority()
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.is_visible()
    }
}

#[pymethods]
impl PyScene {
    fn add_annotation(&mut self, annotation: PyAnnotation) -> PyResult<PyAnnotationHandle> {
        core(self.inner.add_annotation(annotation.0)).map(Into::into)
    }

    fn add_measurement(&mut self, measurement: PyMeasurement) -> PyResult<PyMeasurementHandle> {
        core(self.inner.add_measurement(measurement.0)).map(Into::into)
    }

    fn annotation(&self, handle: PyAnnotationHandle) -> Option<PyAnnotation> {
        self.inner.annotation(handle.0).cloned().map(PyAnnotation)
    }

    fn measurement(&self, handle: PyMeasurementHandle) -> Option<PyMeasurement> {
        self.inner.measurement(handle.0).cloned().map(PyMeasurement)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyAnnotationAnchor>()?;
    module.add_class::<PyMarkerShape>()?;
    module.add_class::<PyMarkerStyle>()?;
    module.add_class::<PyAnnotationKind>()?;
    module.add_class::<PyAnnotation>()?;
    module.add_class::<PyMeasurementKind>()?;
    module.add_class::<PyMeasurement>()
}
