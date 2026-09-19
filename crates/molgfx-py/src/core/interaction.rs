//! Caller-supplied molecular interactions and the glyph they resolve to.
//!
//! Nothing here is classified: a caller supplies the endpoints, the class and
//! the geometry it already computed, and the scene owns the resulting edge.
//! Every picture need is derived from those facts, so the same record always
//! draws the same glyph.

use super::{PyEntityRef, PyStructureHandle};
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

impl From<PyInteractionKind> for molgfx::core::InteractionKind {
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

impl From<molgfx::core::InteractionKind> for PyInteractionKind {
    fn from(value: molgfx::core::InteractionKind) -> Self {
        match value {
            molgfx::core::InteractionKind::HydrogenBond => Self::HydrogenBond,
            molgfx::core::InteractionKind::SaltBridge => Self::SaltBridge,
            molgfx::core::InteractionKind::PiStacking => Self::PiStacking,
            molgfx::core::InteractionKind::Hydrophobic => Self::Hydrophobic,
            molgfx::core::InteractionKind::MetalCoordination => Self::MetalCoordination,
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

impl From<PyInteractionDirection> for molgfx::core::InteractionDirection {
    fn from(value: PyInteractionDirection) -> Self {
        match value {
            PyInteractionDirection::Undirected => Self::Undirected,
            PyInteractionDirection::Forward => Self::Forward,
            PyInteractionDirection::Reverse => Self::Reverse,
        }
    }
}

impl From<molgfx::core::InteractionDirection> for PyInteractionDirection {
    fn from(value: molgfx::core::InteractionDirection) -> Self {
        match value {
            molgfx::core::InteractionDirection::Undirected => Self::Undirected,
            molgfx::core::InteractionDirection::Forward => Self::Forward,
            molgfx::core::InteractionDirection::Reverse => Self::Reverse,
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

impl From<molgfx::core::InteractionPattern> for PyInteractionPattern {
    fn from(value: molgfx::core::InteractionPattern) -> Self {
        match value {
            molgfx::core::InteractionPattern::Solid => Self::Solid,
            molgfx::core::InteractionPattern::Dashes => Self::Dashes,
            molgfx::core::InteractionPattern::Dots => Self::Dots,
            molgfx::core::InteractionPattern::Spring => Self::Spring,
        }
    }
}

/// One already-resolved world-space interaction endpoint.
#[pyclass(name = "InteractionAnchor", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyInteractionAnchor(pub(crate) molgfx::core::InteractionAnchor);

impl From<molgfx::core::InteractionAnchor> for PyInteractionAnchor {
    fn from(value: molgfx::core::InteractionAnchor) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyInteractionAnchor {
    /// A free anchor such as a ring or group centroid.
    #[staticmethod]
    fn world(position: PyVec3) -> PyResult<Self> {
        core(molgfx::core::InteractionAnchor::world(position.0)).map(Self)
    }

    /// An anchor associated with an atom or another scene entity.
    #[staticmethod]
    fn entity(position: PyVec3, entity: PyEntityRef) -> PyResult<Self> {
        core(molgfx::core::InteractionAnchor::entity(
            position.0, entity.0,
        ))
        .map(Self)
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

/// Caller-computed geometry retained for inspection and labels.
#[pyclass(name = "InteractionGeometry", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyInteractionGeometry(pub(crate) molgfx::core::InteractionGeometry);

impl From<molgfx::core::InteractionGeometry> for PyInteractionGeometry {
    fn from(value: molgfx::core::InteractionGeometry) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyInteractionGeometry {
    #[new]
    #[pyo3(signature = (distance_angstrom, angle_degrees=None))]
    fn new(distance_angstrom: f32, angle_degrees: Option<f32>) -> PyResult<Self> {
        core(molgfx::core::InteractionGeometry::new(
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

/// Glyph presentation an interaction resolves to.
#[pyclass(name = "InteractionStyle", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyInteractionStyle(pub(crate) molgfx::core::InteractionStyle);

#[pymethods]
impl PyInteractionStyle {
    #[getter]
    fn color(&self) -> PyRgba8 {
        PyRgba8(self.0.color)
    }

    #[getter]
    fn pattern(&self) -> PyInteractionPattern {
        self.0.pattern.into()
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

    #[getter]
    fn phase_speed_pixels_per_frame(&self) -> f32 {
        self.0.phase_speed_pixels_per_frame
    }

    fn __repr__(&self) -> String {
        format!(
            "InteractionStyle(width={}, opacity={})",
            self.0.width_pixels, self.0.opacity
        )
    }
}

/// One scientific interaction edge owned by the scene.
#[pyclass(name = "InteractionEdge", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyInteraction(pub(crate) molgfx::core::InteractionEdge);

impl From<molgfx::core::InteractionEdge> for PyInteraction {
    fn from(value: molgfx::core::InteractionEdge) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyInteraction {
    /// Creates a visible edge from caller-computed facts.
    #[new]
    fn new(
        owner: PyStructureHandle,
        start: PyInteractionAnchor,
        end: PyInteractionAnchor,
        kind: PyInteractionKind,
        geometry: PyInteractionGeometry,
        provenance: String,
    ) -> PyResult<Self> {
        core(molgfx::core::InteractionEdge::new(
            owner.0,
            start.0,
            end.0,
            kind.into(),
            geometry.0,
            provenance,
        ))
        .map(Self)
    }

    /// Sets source direction without changing endpoint identity.
    fn with_direction(&self, direction: PyInteractionDirection) -> Self {
        Self(self.0.clone().with_direction(direction.into()))
    }

    /// Sets optional occupancy in `[0, 1]`; occupancy maps only to opacity.
    fn with_occupancy(&self, occupancy: f32) -> PyResult<Self> {
        core(self.0.clone().with_occupancy(occupancy)).map(Self)
    }

    /// Sets optional normalized strength in `[0, 1]`; strength maps only to
    /// line width.
    fn with_normalized_strength(&self, strength: f32) -> PyResult<Self> {
        core(self.0.clone().with_normalized_strength(strength)).map(Self)
    }

    /// Sets a deterministic glyph phase speed in pixels per frame.
    fn with_phase_speed(&self, pixels_per_frame: f32) -> PyResult<Self> {
        core(self.0.clone().with_phase_speed(pixels_per_frame)).map(Self)
    }

    /// Supplies visual persistence for an interaction last observed some
    /// frames ago; a zero half-life disables decay.
    fn with_persistence(&self, age_frames: u32, half_life_frames: f32) -> PyResult<Self> {
        core(
            self.0
                .clone()
                .with_persistence(age_frames, half_life_frames),
        )
        .map(Self)
    }

    /// Maps the scientific record to the glyph it draws by default.
    fn resolved_style(&self) -> PyInteractionStyle {
        PyInteractionStyle(self.0.resolved_style())
    }

    /// Structure used for picking and lifecycle ownership.
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
        self.0.kind().into()
    }

    #[getter]
    fn direction(&self) -> PyInteractionDirection {
        self.0.direction().into()
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
    fn phase_speed_pixels_per_frame(&self) -> f32 {
        self.0.phase_speed_pixels_per_frame()
    }

    #[getter]
    fn persistence_age_frames(&self) -> u32 {
        self.0.persistence_age_frames()
    }

    #[getter]
    fn persistence_half_life_frames(&self) -> f32 {
        self.0.persistence_half_life_frames()
    }

    /// Source computation or dataset identifier.
    #[getter]
    fn provenance(&self) -> &str {
        self.0.provenance()
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.visible()
    }

    fn __repr__(&self) -> String {
        format!(
            "InteractionEdge({:?}, provenance={:?})",
            self.0.kind(),
            self.0.provenance()
        )
    }
}
