//! Representation records: appearance, colour, material and clipping state.
//!
//! Numeric geometry knobs stay in the flat array the manifest uses, so the
//! Python view of a representation lines up field for field with the JSON that
//! carries it.

use super::identity::{PyObjectIdentity, PyTargetDescription};
use super::volume::{
    PyPropertyAppearanceDescription, PySegmentationStyleDescription, PySurfaceScalarDescription,
    PyVolumeStyleDescription,
};
use pyo3::prelude::*;

/// Serializable state that changes molecular appearance.
#[pyclass(name = "RepresentationDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyRepresentationDescription(pub(crate) molgfx::core::RepresentationDescription);

#[pymethods]
impl PyRepresentationDescription {
    /// Stable representation slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Generation of the representation handle.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// Stable representation kind name.
    #[getter]
    fn kind(&self) -> String {
        self.0.kind.clone()
    }

    /// Non-process-local target identity.
    #[getter]
    fn target(&self) -> PyTargetDescription {
        PyTargetDescription(self.0.target.clone())
    }

    /// Whether the representation draws.
    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }

    /// Deterministic order; lower values draw first.
    #[getter]
    fn order(&self) -> u16 {
        self.0.order
    }

    /// Colouring state.
    #[getter]
    fn color(&self) -> PyColorDescription {
        PyColorDescription(self.0.color.clone())
    }

    /// Surface response state.
    #[getter]
    fn material(&self) -> PyMaterialDescription {
        PyMaterialDescription(self.0.material.clone())
    }

    /// Numeric geometry knobs in stable order.
    #[getter]
    fn params(&self) -> [f32; 15] {
        self.0.params
    }

    /// Sampled-field connected-component threshold.
    #[getter]
    fn surface_components(&self) -> PySurfaceComponentDescription {
        PySurfaceComponentDescription(self.0.surface_components.clone())
    }

    /// Clipping state.
    #[getter]
    fn clipping(&self) -> PyClipDescription {
        PyClipDescription(self.0.clipping.clone())
    }

    /// Optional reversible variable-radius tube mapping.
    #[getter]
    fn tube_radius_mapping(&self) -> Option<[f32; 4]> {
        self.0.tube_radius_mapping
    }

    /// Optional property-driven opacity and silhouette softness.
    #[getter]
    fn appearance(&self) -> Option<PyPropertyAppearanceDescription> {
        self.0
            .appearance
            .clone()
            .map(PyPropertyAppearanceDescription)
    }

    /// Scalar-volume sampling and transfer state.
    #[getter]
    fn volume(&self) -> PyVolumeStyleDescription {
        PyVolumeStyleDescription(self.0.volume.clone())
    }

    /// Categorical-volume sampling and label styles.
    #[getter]
    fn segmentation(&self) -> PySegmentationStyleDescription {
        PySegmentationStyleDescription(self.0.segmentation.clone())
    }

    /// Optional scalar field sampled over a molecular surface.
    #[getter]
    fn surface_scalar(&self) -> Option<PySurfaceScalarDescription> {
        self.0
            .surface_scalar
            .clone()
            .map(PySurfaceScalarDescription)
    }

    /// Optional typed visual program and its current parameter block.
    #[getter]
    fn visual(&self) -> Option<PyVisualStyleDescription> {
        self.0.visual.clone().map(PyVisualStyleDescription)
    }
}

/// Serializable sampled-surface connected-component policy.
///
/// The variants that carry a measurement are constructed by name — the
/// policy either keeps everything, or sets one minimum expressed as an area,
/// a volume or a voxel count.
#[pyclass(name = "SurfaceComponentDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PySurfaceComponentDescription(
    pub(crate) molgfx::core::SurfaceComponentDescription,
);

#[pymethods]
impl PySurfaceComponentDescription {
    /// Keep every component.
    #[classattr]
    #[pyo3(name = "Disabled")]
    fn disabled_variant() -> Self {
        Self(molgfx::core::SurfaceComponentDescription::Disabled)
    }

    /// Minimum exposed-face area in square ångström.
    #[staticmethod]
    fn area(square_angstrom: f64) -> Self {
        Self(molgfx::core::SurfaceComponentDescription::Area(
            square_angstrom,
        ))
    }

    /// Minimum occupied volume in cubic ångström.
    #[staticmethod]
    fn volume(cubic_angstrom: f64) -> Self {
        Self(molgfx::core::SurfaceComponentDescription::Volume(
            cubic_angstrom,
        ))
    }

    /// Minimum occupied voxel count.
    #[staticmethod]
    fn voxels(count: u64) -> Self {
        Self(molgfx::core::SurfaceComponentDescription::Voxels(count))
    }

    /// `disabled`, `area`, `volume` or `voxels`.
    #[getter]
    fn measure(&self) -> String {
        match self.0 {
            molgfx::core::SurfaceComponentDescription::Disabled => "disabled",
            molgfx::core::SurfaceComponentDescription::Area(_) => "area",
            molgfx::core::SurfaceComponentDescription::Volume(_) => "volume",
            molgfx::core::SurfaceComponentDescription::Voxels(_) => "voxels",
        }
        .to_owned()
    }

    /// Minimum exposed-face area, when the policy measures area.
    #[getter]
    fn minimum_area(&self) -> Option<f64> {
        match self.0 {
            molgfx::core::SurfaceComponentDescription::Area(value) => Some(value),
            _ => None,
        }
    }

    /// Minimum occupied volume, when the policy measures volume.
    #[getter]
    fn minimum_volume(&self) -> Option<f64> {
        match self.0 {
            molgfx::core::SurfaceComponentDescription::Volume(value) => Some(value),
            _ => None,
        }
    }

    /// Minimum occupied voxel count, when the policy counts voxels.
    #[getter]
    fn minimum_voxels(&self) -> Option<u64> {
        match self.0 {
            molgfx::core::SurfaceComponentDescription::Voxels(value) => Some(value),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        format!("SurfaceComponentDescription({})", self.measure())
    }
}

/// Serialized safe visual program and its current parameter block.
#[pyclass(name = "VisualStyleDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyVisualStyleDescription(pub(crate) molgfx::core::VisualStyleDescription);

#[pymethods]
impl PyVisualStyleDescription {
    /// Fixed-width validated instruction stream.
    #[getter]
    fn instructions(&self) -> Vec<PyVisualInstructionDescription> {
        self.0
            .instructions
            .iter()
            .copied()
            .map(PyVisualInstructionDescription)
            .collect()
    }

    /// Output-channel code and result register, per output.
    #[getter]
    fn outputs(&self) -> Vec<[u8; 2]> {
        self.0.outputs.clone()
    }

    /// Referenced atom-property identities in descriptor order.
    #[getter]
    fn properties(&self) -> Vec<PyObjectIdentity> {
        self.0
            .properties
            .iter()
            .copied()
            .map(PyObjectIdentity::from)
            .collect()
    }

    /// Referenced typed attributes in descriptor order.
    #[getter]
    fn attributes(&self) -> Vec<PyVisualAttributeDescription> {
        self.0
            .attributes
            .iter()
            .cloned()
            .map(PyVisualAttributeDescription)
            .collect()
    }

    /// Parameter value-kind codes.
    #[getter]
    fn parameter_kinds(&self) -> Vec<u8> {
        self.0.parameter_kinds.clone()
    }

    /// Default parameter values used by the immutable program.
    #[getter]
    fn parameter_defaults(&self) -> Vec<[f32; 4]> {
        self.0.parameter_defaults.clone()
    }

    /// Current mutable style parameter values.
    #[getter]
    fn parameters(&self) -> Vec<[f32; 4]> {
        self.0.parameters.clone()
    }

    /// Conservative local displacement bound.
    #[getter]
    fn maximum_displacement(&self) -> f32 {
        self.0.maximum_displacement
    }
}

/// One typed visual attribute handle and expected physical layout.
#[pyclass(name = "VisualAttributeDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyVisualAttributeDescription(pub(crate) molgfx::core::VisualAttributeDescription);

#[pymethods]
impl PyVisualAttributeDescription {
    /// Stable attribute slot identity.
    #[getter]
    fn identity(&self) -> PyObjectIdentity {
        PyObjectIdentity(self.0.identity)
    }

    /// `scalar`, `category`, `vector` or `color`.
    #[getter]
    fn kind(&self) -> String {
        self.0.kind.clone()
    }
}

/// One fixed-width visual instruction.
#[pyclass(name = "VisualInstructionDescription", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyVisualInstructionDescription(
    pub(crate) molgfx::core::VisualInstructionDescription,
);

#[pymethods]
impl PyVisualInstructionDescription {
    /// Portable opcode.
    #[getter]
    fn opcode(&self) -> u32 {
        self.0.opcode
    }

    /// Typed value-kind code.
    #[getter]
    fn kind(&self) -> u8 {
        self.0.kind
    }

    /// Source registers.
    #[getter]
    fn operands(&self) -> [u8; 3] {
        self.0.operands
    }

    /// Literal payload or input index.
    #[getter]
    fn data(&self) -> [f32; 4] {
        self.0.data
    }

    /// Earliest evaluation-stage code.
    #[getter]
    fn stage(&self) -> u8 {
        self.0.stage
    }
}

/// Stable description of a colour source and optional scalar ramp.
#[pyclass(name = "ColorDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyColorDescription(pub(crate) molgfx::core::ColorDescription);

#[pymethods]
impl PyColorDescription {
    /// `element`, `chain`, `residue`, `secondary`, `property` or `uniform`.
    #[getter]
    fn mode(&self) -> String {
        self.0.mode.clone()
    }

    /// Uniform or missing colour, when applicable.
    #[getter]
    fn rgba(&self) -> Option<[u8; 4]> {
        self.0.rgba
    }

    /// Property slot row, when applicable.
    #[getter]
    fn property_row(&self) -> Option<u32> {
        self.0.property_row
    }

    /// Property slot generation, when applicable.
    #[getter]
    fn property_generation(&self) -> Option<u32> {
        self.0.property_generation
    }

    /// Numeric ramp stops, when applicable.
    #[getter]
    fn ramp_values(&self) -> Option<[u32; 3]> {
        self.0.ramp_values
    }

    /// Ramp colours, when applicable.
    #[getter]
    fn ramp_colors(&self) -> Option<[[u8; 4]; 3]> {
        self.0.ramp_colors
    }
}

/// Compact material description.
#[pyclass(name = "MaterialDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyMaterialDescription(pub(crate) molgfx::core::MaterialDescription);

#[pymethods]
impl PyMaterialDescription {
    /// Opacity, roughness and specular strength.
    #[getter]
    fn response(&self) -> [f32; 3] {
        self.0.response
    }

    /// Stable material model name.
    #[getter]
    fn model(&self) -> String {
        self.0.model.clone()
    }

    /// Model parameter, such as metalness or anisotropy.
    #[getter]
    fn model_parameter(&self) -> f32 {
        self.0.model_parameter
    }
}

/// World-space clipping and cap description.
#[pyclass(name = "ClipDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyClipDescription(pub(crate) molgfx::core::ClipDescription);

#[pymethods]
impl PyClipDescription {
    /// Active planes as normal xyz plus offset.
    #[getter]
    fn planes(&self) -> Vec<[f32; 4]> {
        self.0.planes.clone()
    }

    /// `open` or `solid`.
    #[getter]
    fn cap(&self) -> String {
        self.0.cap.clone()
    }
}
