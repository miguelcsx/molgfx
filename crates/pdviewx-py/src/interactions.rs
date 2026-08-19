//! Python adapters for caller-computed molecular interaction glyphs.

use crate::core::{PyEntityRef, PyInteractionHandle, PyScene, PyStructureHandle};
use crate::error::core;
use crate::math::{PyRgba8, PyVec3};
use pyo3::prelude::*;

#[pyclass(name = "InteractionKind", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyInteractionKind {
    HydrogenBond,
    SaltBridge,
    PiStacking,
    Hydrophobic,
    MetalCoordination,
}

impl From<PyInteractionKind> for pdviewx::InteractionKind {
    fn from(value: PyInteractionKind) -> Self {
        match value {
            PyInteractionKind::HydrogenBond => Self::HydrogenBond,
            PyInteractionKind::SaltBridge => Self::SaltBridge,
            PyInteractionKind::PiStacking => Self::PiStacking,
            PyInteractionKind::Hydrophobic => Self::Hydrophobic,
            PyInteractionKind::MetalCoordination => Self::MetalCoordination,
        }
    }
}

#[pyclass(name = "InteractionDirection", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyInteractionDirection {
    Undirected,
    Forward,
    Reverse,
}

impl From<PyInteractionDirection> for pdviewx::InteractionDirection {
    fn from(value: PyInteractionDirection) -> Self {
        match value {
            PyInteractionDirection::Undirected => Self::Undirected,
            PyInteractionDirection::Forward => Self::Forward,
            PyInteractionDirection::Reverse => Self::Reverse,
        }
    }
}

#[pyclass(name = "InteractionPattern", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyInteractionPattern {
    Solid,
    Dashes,
    Dots,
    Spring,
}

#[pyclass(name = "InteractionGeometry", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyInteractionGeometry(pub(crate) pdviewx::InteractionGeometry);

#[pymethods]
impl PyInteractionGeometry {
    #[new]
    fn new(distance_angstrom: f32, angle_degrees: Option<f32>) -> PyResult<Self> {
        core(pdviewx::InteractionGeometry::new(
            distance_angstrom,
            angle_degrees,
        ))
        .map(Self)
    }

    #[getter]
    fn distance_angstrom(&self) -> f32 {
        self.0.distance_angstrom()
    }

    #[getter]
    fn angle_degrees(&self) -> Option<f32> {
        self.0.angle_degrees()
    }
}

#[pyclass(name = "InteractionStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyInteractionStyle(pub(crate) pdviewx::InteractionStyle);

#[pymethods]
impl PyInteractionStyle {
    #[getter]
    fn color(&self) -> PyRgba8 {
        self.0.color.into()
    }

    #[getter]
    fn pattern(&self) -> PyInteractionPattern {
        match self.0.pattern {
            pdviewx::InteractionPattern::Solid => PyInteractionPattern::Solid,
            pdviewx::InteractionPattern::Dashes => PyInteractionPattern::Dashes,
            pdviewx::InteractionPattern::Dots => PyInteractionPattern::Dots,
            pdviewx::InteractionPattern::Spring => PyInteractionPattern::Spring,
        }
    }

    #[getter]
    fn width_pixels(&self) -> f32 {
        self.0.width_pixels
    }
    #[getter]
    fn opacity(&self) -> f32 {
        self.0.opacity
    }
    #[getter]
    fn period_pixels(&self) -> f32 {
        self.0.period_pixels
    }
    #[getter]
    fn duty_cycle(&self) -> f32 {
        self.0.duty_cycle
    }
}

#[pyclass(name = "InteractionAnchor", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyInteractionAnchor(pub(crate) pdviewx::InteractionAnchor);

#[pymethods]
impl PyInteractionAnchor {
    #[staticmethod]
    fn world(position: PyVec3) -> PyResult<Self> {
        core(pdviewx::InteractionAnchor::world(position.0)).map(Self)
    }

    #[staticmethod]
    fn entity(position: PyVec3, entity: PyEntityRef) -> PyResult<Self> {
        core(pdviewx::InteractionAnchor::entity(position.0, entity.0)).map(Self)
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

#[pyclass(name = "InteractionEdge", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyInteractionEdge(pub(crate) pdviewx::InteractionEdge);

#[pymethods]
impl PyInteractionEdge {
    #[new]
    fn new(
        owner: PyStructureHandle,
        start: PyInteractionAnchor,
        end: PyInteractionAnchor,
        kind: PyInteractionKind,
        geometry: PyInteractionGeometry,
        provenance: &str,
    ) -> PyResult<Self> {
        core(pdviewx::InteractionEdge::new(
            owner.0,
            start.0,
            end.0,
            kind.into(),
            geometry.0,
            provenance,
        ))
        .map(Self)
    }

    fn with_direction(&self, direction: PyInteractionDirection) -> Self {
        Self(self.0.clone().with_direction(direction.into()))
    }
    fn with_occupancy(&self, value: f32) -> PyResult<Self> {
        core(self.0.clone().with_occupancy(value)).map(Self)
    }
    fn with_normalized_strength(&self, value: f32) -> PyResult<Self> {
        core(self.0.clone().with_normalized_strength(value)).map(Self)
    }
    fn with_phase_speed(&self, value: f32) -> PyResult<Self> {
        core(self.0.clone().with_phase_speed(value)).map(Self)
    }
    fn with_persistence(&self, age: u32, half_life: f32) -> PyResult<Self> {
        core(self.0.clone().with_persistence(age, half_life)).map(Self)
    }

    #[getter]
    fn owner(&self) -> PyStructureHandle {
        self.0.owner().into()
    }
    #[getter]
    fn start(&self) -> PyInteractionAnchor {
        PyInteractionAnchor(self.0.start())
    }
    #[getter]
    fn end(&self) -> PyInteractionAnchor {
        PyInteractionAnchor(self.0.end())
    }
    #[getter]
    fn kind(&self) -> PyInteractionKind {
        match self.0.kind() {
            pdviewx::InteractionKind::HydrogenBond => PyInteractionKind::HydrogenBond,
            pdviewx::InteractionKind::SaltBridge => PyInteractionKind::SaltBridge,
            pdviewx::InteractionKind::PiStacking => PyInteractionKind::PiStacking,
            pdviewx::InteractionKind::Hydrophobic => PyInteractionKind::Hydrophobic,
            pdviewx::InteractionKind::MetalCoordination => PyInteractionKind::MetalCoordination,
        }
    }
    #[getter]
    fn direction(&self) -> PyInteractionDirection {
        match self.0.direction() {
            pdviewx::InteractionDirection::Undirected => PyInteractionDirection::Undirected,
            pdviewx::InteractionDirection::Forward => PyInteractionDirection::Forward,
            pdviewx::InteractionDirection::Reverse => PyInteractionDirection::Reverse,
        }
    }
    #[getter]
    fn geometry(&self) -> PyInteractionGeometry {
        PyInteractionGeometry(self.0.geometry())
    }
    #[getter]
    fn occupancy(&self) -> Option<f32> {
        self.0.occupancy()
    }
    #[getter]
    fn normalized_strength(&self) -> Option<f32> {
        self.0.normalized_strength()
    }
    #[getter]
    fn provenance(&self) -> String {
        self.0.provenance().to_owned()
    }
    fn resolved_style(&self) -> PyInteractionStyle {
        PyInteractionStyle(self.0.resolved_style())
    }
}

#[pymethods]
impl PyScene {
    fn add_interaction(&mut self, interaction: PyInteractionEdge) -> PyResult<PyInteractionHandle> {
        core(self.inner.add_interaction(interaction.0)).map(Into::into)
    }

    fn interaction(&self, handle: PyInteractionHandle) -> Option<PyInteractionEdge> {
        self.inner
            .interaction(handle.0)
            .cloned()
            .map(PyInteractionEdge)
    }

    #[getter]
    fn interaction_count(&self) -> usize {
        self.inner.interaction_count()
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyInteractionKind>()?;
    module.add_class::<PyInteractionDirection>()?;
    module.add_class::<PyInteractionPattern>()?;
    module.add_class::<PyInteractionGeometry>()?;
    module.add_class::<PyInteractionStyle>()?;
    module.add_class::<PyInteractionAnchor>()?;
    module.add_class::<PyInteractionEdge>()
}
