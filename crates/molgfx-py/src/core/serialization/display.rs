//! Records for caller-authored presentation: guides, notes, measurements,
//! interaction facts and ligand candidates.
//!
//! These are the descriptions a caller supplies rather than the ones a scene
//! derives, so they carry both the authored geometry and the presentation
//! policy that goes with it.

use super::identity::{PyAnchorDescription, PyObjectIdentity};
use pyo3::prelude::*;

/// Serialized guide style.
#[pyclass(name = "GuideStyleDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyGuideStyleDescription(pub(crate) molgfx::core::GuideStyleDescription);

#[pymethods]
impl PyGuideStyleDescription {
    /// Display colour.
    #[getter]
    fn color(&self) -> [u8; 4] {
        self.0.color
    }

    /// Stable pattern name.
    #[getter]
    fn pattern(&self) -> String {
        self.0.pattern.clone()
    }

    /// Width in physical pixels.
    #[getter]
    fn width_pixels(&self) -> f32 {
        self.0.width_pixels
    }

    /// Final opacity.
    #[getter]
    fn opacity(&self) -> f32 {
        self.0.opacity
    }

    /// Repetition period in pixels.
    #[getter]
    fn period_pixels(&self) -> f32 {
        self.0.period_pixels
    }

    /// Mark duty cycle.
    #[getter]
    fn duty_cycle(&self) -> f32 {
        self.0.duty_cycle
    }

    /// Stable cap name.
    #[getter]
    fn cap(&self) -> String {
        self.0.cap.clone()
    }

    /// Arrowhead size in pixels.
    #[getter]
    fn arrow_pixels(&self) -> f32 {
        self.0.arrow_pixels
    }
}

/// One caller-authored guide.
#[pyclass(name = "GuideDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyGuideDescription(pub(crate) molgfx::core::GuideDescription);

#[pymethods]
impl PyGuideDescription {
    /// Stable slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Slot generation.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// Owning structure.
    #[getter]
    fn owner(&self) -> PyObjectIdentity {
        PyObjectIdentity(self.0.owner)
    }

    /// Model-space start.
    #[getter]
    fn start(&self) -> [f32; 3] {
        self.0.start
    }

    /// Model-space end.
    #[getter]
    fn end(&self) -> [f32; 3] {
        self.0.end
    }

    /// Guide presentation.
    #[getter]
    fn style(&self) -> PyGuideStyleDescription {
        PyGuideStyleDescription(self.0.style.clone())
    }

    /// Render visibility.
    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }
}

/// Serialized marker style.
#[pyclass(name = "MarkerStyleDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyMarkerStyleDescription(pub(crate) molgfx::core::MarkerStyleDescription);

#[pymethods]
impl PyMarkerStyleDescription {
    /// Display colour.
    #[getter]
    fn color(&self) -> [u8; 4] {
        self.0.color
    }

    /// Physical-pixel radius.
    #[getter]
    fn radius_pixels(&self) -> f32 {
        self.0.radius_pixels
    }

    /// Stable shape name.
    #[getter]
    fn shape(&self) -> String {
        self.0.shape.clone()
    }
}

/// One persistent annotation.
#[pyclass(name = "AnnotationDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyAnnotationDescription(pub(crate) molgfx::core::AnnotationDescription);

#[pymethods]
impl PyAnnotationDescription {
    /// Stable slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Slot generation.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// Owning structure.
    #[getter]
    fn owner(&self) -> PyObjectIdentity {
        PyObjectIdentity(self.0.owner)
    }

    /// Stable annotation kind.
    #[getter]
    fn kind(&self) -> String {
        self.0.kind.clone()
    }

    /// Optional geometric anchor.
    #[getter]
    fn anchor(&self) -> Option<PyAnchorDescription> {
        self.0.anchor.clone().map(PyAnchorDescription)
    }

    /// Optional region selection.
    #[getter]
    fn region(&self) -> Option<PyObjectIdentity> {
        self.0.region.map(PyObjectIdentity)
    }

    /// Human-authored text.
    #[getter]
    fn text(&self) -> String {
        self.0.text.clone()
    }

    /// Marker presentation.
    #[getter]
    fn marker(&self) -> PyMarkerStyleDescription {
        PyMarkerStyleDescription(self.0.marker.clone())
    }

    /// Decluttering priority.
    #[getter]
    fn priority(&self) -> i16 {
        self.0.priority
    }

    /// Render visibility.
    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }
}

/// One persistent caller-computed measurement.
#[pyclass(name = "MeasurementDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyMeasurementDescription(pub(crate) molgfx::core::MeasurementDescription);

#[pymethods]
impl PyMeasurementDescription {
    /// Stable slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Slot generation.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// Owning structure.
    #[getter]
    fn owner(&self) -> PyObjectIdentity {
        PyObjectIdentity(self.0.owner)
    }

    /// Stable measurement kind.
    #[getter]
    fn kind(&self) -> String {
        self.0.kind.clone()
    }

    /// Ordered source anchors.
    #[getter]
    fn anchors(&self) -> Vec<PyAnchorDescription> {
        self.0
            .anchors
            .iter()
            .cloned()
            .map(PyAnchorDescription)
            .collect()
    }

    /// Caller-computed value.
    #[getter]
    fn value(&self) -> f32 {
        self.0.value
    }

    /// Deterministic display label.
    #[getter]
    fn label(&self) -> String {
        self.0.label.clone()
    }

    /// Source computation identifier.
    #[getter]
    fn provenance(&self) -> String {
        self.0.provenance.clone()
    }

    /// Decluttering priority.
    #[getter]
    fn priority(&self) -> i16 {
        self.0.priority
    }

    /// Render visibility.
    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }
}

/// One caller-computed interaction fact.
#[pyclass(name = "InteractionDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyInteractionDescription(pub(crate) molgfx::core::InteractionDescription);

#[pymethods]
impl PyInteractionDescription {
    /// Stable interaction row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Interaction handle generation.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// Owning structure identity.
    #[getter]
    fn owner(&self) -> PyObjectIdentity {
        PyObjectIdentity(self.0.owner)
    }

    /// First interaction endpoint.
    #[getter]
    fn start(&self) -> PyAnchorDescription {
        PyAnchorDescription(self.0.start.clone())
    }

    /// Second interaction endpoint.
    #[getter]
    fn end(&self) -> PyAnchorDescription {
        PyAnchorDescription(self.0.end.clone())
    }

    /// Stable interaction class name.
    #[getter]
    fn kind(&self) -> String {
        self.0.kind.clone()
    }

    /// Stable direction name.
    #[getter]
    fn direction(&self) -> String {
        self.0.direction.clone()
    }

    /// Caller-computed distance in ångström.
    #[getter]
    fn distance_angstrom(&self) -> f32 {
        self.0.distance_angstrom
    }

    /// Optional caller-computed angle in degrees.
    #[getter]
    fn angle_degrees(&self) -> Option<f32> {
        self.0.angle_degrees
    }

    /// Optional occupancy.
    #[getter]
    fn occupancy(&self) -> Option<f32> {
        self.0.occupancy
    }

    /// Optional normalized line-strength value.
    #[getter]
    fn normalized_strength(&self) -> Option<f32> {
        self.0.normalized_strength
    }

    /// Optional presentation phase speed in pixels per frame.
    #[getter]
    fn phase_speed_pixels_per_frame(&self) -> f32 {
        self.0.phase_speed_pixels_per_frame
    }

    /// Caller-computed age for presentation persistence decay.
    #[getter]
    fn persistence_age_frames(&self) -> u32 {
        self.0.persistence_age_frames
    }

    /// Presentation half-life in frames; zero disables visual decay.
    #[getter]
    fn persistence_half_life_frames(&self) -> f32 {
        self.0.persistence_half_life_frames
    }

    /// Source computation or dataset identifier.
    #[getter]
    fn provenance(&self) -> String {
        self.0.provenance.clone()
    }

    /// Whether the edge participates in rendering.
    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }
}

/// One rigid ligand pose in a scene manifest.
#[pyclass(name = "LigandPoseDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyLigandPoseDescription(pub(crate) molgfx::core::LigandPoseDescription);

#[pymethods]
impl PyLigandPoseDescription {
    /// Destination of the template origin.
    #[getter]
    fn translation(&self) -> [f32; 3] {
        self.0.translation
    }

    /// Unit rotation applied before translation.
    #[getter]
    fn orientation(&self) -> [f32; 4] {
        self.0.orientation
    }

    /// Candidate display colour.
    #[getter]
    fn color(&self) -> [u8; 4] {
        self.0.color
    }

    /// Candidate opacity.
    #[getter]
    fn opacity(&self) -> f32 {
        self.0.opacity
    }
}

/// One compact reusable topology and its rigid occurrences.
#[pyclass(name = "LigandPoseBatchDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyLigandPoseBatchDescription(pub(crate) molgfx::core::LigandPoseBatchDescription);

#[pymethods]
impl PyLigandPoseBatchDescription {
    /// Stable batch row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Stable batch generation.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// Owning structure identity.
    #[getter]
    fn owner(&self) -> PyObjectIdentity {
        PyObjectIdentity(self.0.owner)
    }

    /// Local atom centres relative to the template origin.
    #[getter]
    fn atoms(&self) -> Vec<[f32; 3]> {
        self.0.atoms.clone()
    }

    /// Zero-based topology edges.
    #[getter]
    fn bonds(&self) -> Vec<[u32; 2]> {
        self.0.bonds.clone()
    }

    /// Sphere radius in ångström.
    #[getter]
    fn atom_radius(&self) -> f32 {
        self.0.atom_radius
    }

    /// Capsule radius in ångström.
    #[getter]
    fn bond_radius(&self) -> f32 {
        self.0.bond_radius
    }

    /// Compact rigid candidate column.
    #[getter]
    fn poses(&self) -> Vec<PyLigandPoseDescription> {
        self.0
            .poses
            .iter()
            .cloned()
            .map(PyLigandPoseDescription)
            .collect()
    }

    /// Whether this batch participates in rendering.
    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }
}
