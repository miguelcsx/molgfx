//! Volume, property and scalar-field records.
//!
//! A volume manifest keeps the grid identity and its content fingerprint, not
//! the samples: the values stay caller-owned and are addressed by hash. The
//! same split governs per-atom property columns and the scalar fields sampled
//! over a molecular boundary.

use super::identity::PyObjectIdentity;
use pyo3::prelude::*;

/// Source grid identity and dimensions. Values remain caller-owned.
#[pyclass(name = "VolumeDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyVolumeDescription(pub(crate) molgfx::core::VolumeDescription);

#[pymethods]
impl PyVolumeDescription {
    /// Stable slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Generation of the volume handle.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// Grid dimensions.
    #[getter]
    fn dimensions(&self) -> [u32; 3] {
        self.0.dimensions
    }

    /// Scalar range; categorical volumes use `[0, 0]`.
    #[getter]
    fn range(&self) -> [f32; 2] {
        self.0.range
    }

    /// Column-major voxel-to-world transform.
    #[getter]
    fn voxel_to_world(&self) -> [f32; 16] {
        self.0.voxel_to_world
    }

    /// FNV-1a fingerprint over caller-owned samples or labels.
    #[getter]
    fn content_hash(&self) -> u64 {
        self.0.content_hash
    }

    /// Temporal occupancy source, when this is not a static grid.
    #[getter]
    fn occupancy(&self) -> Option<PyOccupancyDescription> {
        self.0.occupancy.clone().map(PyOccupancyDescription)
    }
}

/// Persistent declaration for a GPU-resident temporal occupancy field.
#[pyclass(name = "OccupancyDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyOccupancyDescription(pub(crate) molgfx::core::OccupancyDescription);

#[pymethods]
impl PyOccupancyDescription {
    /// Structure providing the sampled coordinate rows.
    #[getter]
    fn structure(&self) -> PyObjectIdentity {
        PyObjectIdentity(self.0.structure)
    }

    /// Sorted structure-local atom rows deposited per sample.
    #[getter]
    fn atom_rows(&self) -> Vec<u32> {
        self.0.atom_rows.clone()
    }

    /// Column-major voxel-to-model transform.
    #[getter]
    fn voxel_to_model(&self) -> [f32; 16] {
        self.0.voxel_to_model
    }

    /// Multiplicative history decay.
    #[getter]
    fn decay(&self) -> f32 {
        self.0.decay
    }

    /// Mass deposited by each selected atom.
    #[getter]
    fn deposit(&self) -> f32 {
        self.0.deposit
    }

    /// Saturation ceiling.
    #[getter]
    fn maximum(&self) -> f32 {
        self.0.maximum
    }
}

/// Serializable scalar-volume sampling state.
#[pyclass(name = "VolumeStyleDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyVolumeStyleDescription(pub(crate) molgfx::core::VolumeStyleDescription);

#[pymethods]
impl PyVolumeStyleDescription {
    /// Stable rendering name.
    #[getter]
    fn rendering(&self) -> String {
        self.0.rendering.clone()
    }

    /// Ordered transfer stops.
    #[getter]
    fn transfer(&self) -> Vec<PyVolumeTransferPointDescription> {
        self.0
            .transfer
            .iter()
            .cloned()
            .map(PyVolumeTransferPointDescription)
            .collect()
    }

    /// Global optical-density multiplier.
    #[getter]
    fn opacity_scale(&self) -> f32 {
        self.0.opacity_scale
    }

    /// Ray-step scale.
    #[getter]
    fn step_scale(&self) -> f32 {
        self.0.step_scale
    }

    /// Optional world-space sampling plane as normal xyz plus offset.
    #[getter]
    fn slice(&self) -> Option<[f32; 4]> {
        self.0.slice
    }

    /// Optional half-open voxel crop.
    #[getter]
    fn region(&self) -> Option<PyRegionDescription> {
        self.0.region.map(PyRegionDescription)
    }
}

/// One transfer-function stop in a volume representation.
#[pyclass(name = "VolumeTransferPointDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyVolumeTransferPointDescription(
    pub(crate) molgfx::core::VolumeTransferPointDescription,
);

#[pymethods]
impl PyVolumeTransferPointDescription {
    /// Scalar value.
    #[getter]
    fn value(&self) -> f32 {
        self.0.value
    }

    /// Stop colour.
    #[getter]
    fn color(&self) -> [u8; 4] {
        self.0.color
    }

    /// Stop opacity.
    #[getter]
    fn opacity(&self) -> f32 {
        self.0.opacity
    }
}

/// Serializable categorical-volume sampling state.
#[pyclass(name = "SegmentationStyleDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PySegmentationStyleDescription(
    pub(crate) molgfx::core::SegmentationStyleDescription,
);

#[pymethods]
impl PySegmentationStyleDescription {
    /// Exact label styles.
    #[getter]
    fn styles(&self) -> Vec<PySegmentStyleDescription> {
        self.0
            .styles
            .iter()
            .cloned()
            .map(PySegmentStyleDescription)
            .collect()
    }

    /// Global opacity multiplier.
    #[getter]
    fn opacity_scale(&self) -> f32 {
        self.0.opacity_scale
    }

    /// Ray-step scale.
    #[getter]
    fn step_scale(&self) -> f32 {
        self.0.step_scale
    }

    /// Optional world-space sampling plane as normal xyz plus offset.
    #[getter]
    fn slice(&self) -> Option<[f32; 4]> {
        self.0.slice
    }

    /// Optional half-open voxel crop.
    #[getter]
    fn region(&self) -> Option<PyRegionDescription> {
        self.0.region.map(PyRegionDescription)
    }
}

/// One exact categorical label style.
#[pyclass(name = "SegmentStyleDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PySegmentStyleDescription(pub(crate) molgfx::core::SegmentStyleDescription);

#[pymethods]
impl PySegmentStyleDescription {
    /// Integer label.
    #[getter]
    fn label(&self) -> u32 {
        self.0.label
    }

    /// Display colour.
    #[getter]
    fn color(&self) -> [u8; 4] {
        self.0.color
    }

    /// Label opacity.
    #[getter]
    fn opacity(&self) -> f32 {
        self.0.opacity
    }
}

/// Serializable half-open voxel region.
#[pyclass(name = "RegionDescription", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyRegionDescription(pub(crate) molgfx::core::RegionDescription);

#[pymethods]
impl PyRegionDescription {
    /// Inclusive minimum voxel index.
    #[getter]
    fn minimum(&self) -> [u32; 3] {
        self.0.minimum
    }

    /// Exclusive maximum voxel index.
    #[getter]
    fn maximum(&self) -> [u32; 3] {
        self.0.maximum
    }
}

/// Serializable scalar-field semantics carried by a caller-owned property.
#[pyclass(name = "ScalarSemanticsDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyScalarSemanticsDescription(pub(crate) molgfx::core::ScalarSemanticsDescription);

#[pymethods]
impl PyScalarSemanticsDescription {
    /// `rank` or `quantity`.
    #[getter]
    fn kind(&self) -> String {
        self.0.kind.clone()
    }

    /// Quantity name, when calibrated.
    #[getter]
    fn name(&self) -> Option<String> {
        self.0.name.clone()
    }

    /// Quantity units, when calibrated.
    #[getter]
    fn units(&self) -> Option<String> {
        self.0.units.clone()
    }

    /// Upstream method or dataset identifier, when calibrated.
    #[getter]
    fn provenance(&self) -> Option<String> {
        self.0.provenance.clone()
    }
}

/// One caller-owned atom property column and its mismatch fingerprint.
#[pyclass(name = "AtomPropertyDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyAtomPropertyDescription(pub(crate) molgfx::core::AtomPropertyDescription);

#[pymethods]
impl PyAtomPropertyDescription {
    /// Stable property slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Property handle generation.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// Owning structure identity.
    #[getter]
    fn owner(&self) -> PyObjectIdentity {
        PyObjectIdentity(self.0.owner)
    }

    /// Caller-defined property name.
    #[getter]
    fn name(&self) -> String {
        self.0.name.clone()
    }

    /// Number of source atom values, including missing values.
    #[getter]
    fn length(&self) -> u64 {
        self.0.length
    }

    /// Finite display domain.
    #[getter]
    fn finite_domain(&self) -> [f32; 2] {
        self.0.finite_domain
    }

    /// Stable primitive meaning name.
    #[getter]
    fn meaning(&self) -> String {
        self.0.meaning.clone()
    }

    /// Scalar-field semantic metadata.
    #[getter]
    fn semantics(&self) -> PyScalarSemanticsDescription {
        PyScalarSemanticsDescription(self.0.semantics.clone())
    }

    /// FNV-1a fingerprint over value bit patterns.
    #[getter]
    fn content_hash(&self) -> u64 {
        self.0.content_hash
    }
}

/// Reversible property-to-appearance mapping.
#[pyclass(name = "PropertyAppearanceDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyPropertyAppearanceDescription(
    pub(crate) molgfx::core::PropertyAppearanceDescription,
);

#[pymethods]
impl PyPropertyAppearanceDescription {
    /// Property slot identity.
    #[getter]
    fn property(&self) -> PyObjectIdentity {
        PyObjectIdentity(self.0.property)
    }

    /// Primitive input domain.
    #[getter]
    fn domain(&self) -> [f32; 2] {
        self.0.domain
    }

    /// Opacity at the domain endpoints.
    #[getter]
    fn opacity(&self) -> [f32; 2] {
        self.0.opacity
    }

    /// Silhouette softness at the domain endpoints.
    #[getter]
    fn softness_pixels(&self) -> [f32; 2] {
        self.0.softness_pixels
    }

    /// Missing-value response: opacity and softness.
    #[getter]
    fn missing(&self) -> [f32; 2] {
        self.0.missing
    }
}

/// Serializable scalar field sampled over a molecular boundary.
#[pyclass(name = "SurfaceScalarDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PySurfaceScalarDescription(pub(crate) molgfx::core::SurfaceScalarDescription);

#[pymethods]
impl PySurfaceScalarDescription {
    /// Scalar volume slot identity.
    #[getter]
    fn field(&self) -> PyObjectIdentity {
        PyObjectIdentity(self.0.field)
    }

    /// Three ramp values encoded as IEEE-754 bits.
    #[getter]
    fn ramp_values(&self) -> [u32; 3] {
        self.0.ramp_values
    }

    /// Three ramp colours.
    #[getter]
    fn ramp_colors(&self) -> [[u8; 4]; 3] {
        self.0.ramp_colors
    }

    /// Optional contour interval and width in pixels.
    #[getter]
    fn contours(&self) -> Option<[f32; 2]> {
        self.0.contours
    }

    /// Sampling displacement along the surface normal.
    #[getter]
    fn sample_offset_angstrom(&self) -> f32 {
        self.0.sample_offset_angstrom
    }
}
