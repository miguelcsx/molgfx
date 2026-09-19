//! Composition records: what a scene holds, in draw order.
//!
//! [`SceneDescription`] is the whole composition in one value, and the records
//! beside it describe the parts a caller placed — a structure and the
//! fingerprint that detects a later mismatch, a selection expanded into stable
//! structure-local atom rows, and the quick counts for the tables kept out of
//! JSON.

use super::display::{
    PyAnnotationDescription, PyGuideDescription, PyInteractionDescription,
    PyLigandPoseBatchDescription, PyMeasurementDescription,
};
use super::generic::{
    PyAttributeDescription, PyDomainVisualDescription, PyInstanceBatchDescription,
    PyPointBatchDescription, PyRelationBatchDescription,
};
use super::identity::PySelectionMask;
use super::representation::PyRepresentationDescription;
use super::volume::{PyAtomPropertyDescription, PyVolumeDescription};
use crate::pending_surface::descriptions::{
    PyMeshDescription, PyMeshInstanceDescription, PyOverlayDescription, PyPrimitiveDescription,
};
use pyo3::prelude::*;

/// JSON-compatible, reproducible description of a scene composition.
#[pyclass(name = "SceneDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PySceneDescription(pub(crate) molgfx::core::SceneDescription);

#[pymethods]
impl PySceneDescription {
    /// Manifest schema version.
    #[getter]
    fn schema(&self) -> u16 {
        self.0.schema
    }

    /// Engine format identifier, kept separate from the package version.
    #[getter]
    fn engine(&self) -> String {
        self.0.engine.clone()
    }

    /// Caller-supplied structures referenced by source id and coordinate hash.
    #[getter]
    fn structures(&self) -> Vec<PyStructureDescription> {
        self.0
            .structures
            .iter()
            .cloned()
            .map(PyStructureDescription)
            .collect()
    }

    /// Structure-scoped atom selections.
    #[getter]
    fn selections(&self) -> Vec<PySelectionDescription> {
        self.0
            .selections
            .iter()
            .cloned()
            .map(PySelectionDescription)
            .collect()
    }

    /// Caller-owned per-atom scalar columns and their content fingerprints.
    #[getter]
    fn atom_properties(&self) -> Vec<PyAtomPropertyDescription> {
        self.0
            .atom_properties
            .iter()
            .cloned()
            .map(PyAtomPropertyDescription)
            .collect()
    }

    /// Molecular and volume representations in draw order.
    #[getter]
    fn representations(&self) -> Vec<PyRepresentationDescription> {
        self.0
            .representations
            .iter()
            .cloned()
            .map(PyRepresentationDescription)
            .collect()
    }

    /// Resident scalar volume identities and ranges.
    #[getter]
    fn volumes(&self) -> Vec<PyVolumeDescription> {
        self.0
            .volumes
            .iter()
            .cloned()
            .map(PyVolumeDescription)
            .collect()
    }

    /// Resident categorical volume identities.
    #[getter]
    fn segmentations(&self) -> Vec<PyVolumeDescription> {
        self.0
            .segmentations
            .iter()
            .cloned()
            .map(PyVolumeDescription)
            .collect()
    }

    /// Caller mesh fingerprints and presentation state.
    #[getter]
    fn meshes(&self) -> Vec<PyMeshDescription> {
        self.0
            .meshes
            .iter()
            .cloned()
            .map(PyMeshDescription)
            .collect()
    }

    /// Transform-only occurrences of shared meshes.
    #[getter]
    fn mesh_instances(&self) -> Vec<PyMeshInstanceDescription> {
        self.0
            .mesh_instances
            .iter()
            .cloned()
            .map(PyMeshInstanceDescription)
            .collect()
    }

    /// Caller-authored analytic primitive payloads.
    #[getter]
    fn primitives(&self) -> Vec<PyPrimitiveDescription> {
        self.0
            .primitives
            .iter()
            .cloned()
            .map(PyPrimitiveDescription)
            .collect()
    }

    /// Compact reusable-topology ligand candidate batches.
    #[getter]
    fn ligand_pose_batches(&self) -> Vec<PyLigandPoseBatchDescription> {
        self.0
            .ligand_pose_batches
            .iter()
            .cloned()
            .map(PyLigandPoseBatchDescription)
            .collect()
    }

    /// Generic 12-byte point batches with external payloads.
    #[getter]
    fn point_batches(&self) -> Vec<PyPointBatchDescription> {
        self.0
            .point_batches
            .iter()
            .cloned()
            .map(PyPointBatchDescription)
            .collect()
    }

    /// Shared-template 32-byte rigid-instance batches.
    #[getter]
    fn instance_batches(&self) -> Vec<PyInstanceBatchDescription> {
        self.0
            .instance_batches
            .iter()
            .cloned()
            .map(PyInstanceBatchDescription)
            .collect()
    }

    /// Generic typed attribute columns.
    #[getter]
    fn attributes(&self) -> Vec<PyAttributeDescription> {
        self.0
            .attributes
            .iter()
            .cloned()
            .map(PyAttributeDescription)
            .collect()
    }

    /// Generic spatial relation batches.
    #[getter]
    fn relation_batches(&self) -> Vec<PyRelationBatchDescription> {
        self.0
            .relation_batches
            .iter()
            .cloned()
            .map(PyRelationBatchDescription)
            .collect()
    }

    /// Declarative visual programs attached to generic domains.
    #[getter]
    fn domain_visuals(&self) -> Vec<PyDomainVisualDescription> {
        self.0
            .domain_visuals
            .iter()
            .cloned()
            .map(PyDomainVisualDescription)
            .collect()
    }

    /// Depth-independent screen overlays.
    #[getter]
    fn overlays(&self) -> Vec<PyOverlayDescription> {
        self.0
            .overlays
            .iter()
            .cloned()
            .map(PyOverlayDescription)
            .collect()
    }

    /// Caller-authored analytic guide payloads.
    #[getter]
    fn guides(&self) -> Vec<PyGuideDescription> {
        self.0
            .guides
            .iter()
            .cloned()
            .map(PyGuideDescription)
            .collect()
    }

    /// Caller-computed interaction facts.
    #[getter]
    fn interactions(&self) -> Vec<PyInteractionDescription> {
        self.0
            .interactions
            .iter()
            .cloned()
            .map(PyInteractionDescription)
            .collect()
    }

    /// Persistent human-authored annotations.
    #[getter]
    fn annotations(&self) -> Vec<PyAnnotationDescription> {
        self.0
            .annotations
            .iter()
            .cloned()
            .map(PyAnnotationDescription)
            .collect()
    }

    /// Persistent caller-computed measurements.
    #[getter]
    fn measurements(&self) -> Vec<PyMeasurementDescription> {
        self.0
            .measurements
            .iter()
            .cloned()
            .map(PyMeasurementDescription)
            .collect()
    }

    /// Quick counts for dense or caller-owned tables.
    #[getter]
    fn tables(&self) -> PyTableCounts {
        PyTableCounts(self.0.tables)
    }
}

/// One placed source structure and its mismatch-detection fingerprint.
#[pyclass(name = "StructureDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyStructureDescription(pub(crate) molgfx::core::StructureDescription);

#[pymethods]
impl PyStructureDescription {
    /// Stable slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Generation of the placed-structure handle.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// Caller-owned global dataset identity.
    #[getter]
    fn dataset_id(&self) -> u64 {
        self.0.dataset_id
    }

    /// Entry id, when the source carries one.
    #[getter]
    fn source_id(&self) -> Option<String> {
        self.0.source_id.clone()
    }

    /// Source title, when available.
    #[getter]
    fn title(&self) -> Option<String> {
        self.0.title.clone()
    }

    /// Source experimental method, when available.
    #[getter]
    fn method(&self) -> Option<String> {
        self.0.method.clone()
    }

    /// Source resolution, when available.
    #[getter]
    fn resolution(&self) -> Option<f32> {
        self.0.resolution
    }

    /// Number of active atom rows.
    #[getter]
    fn atom_count(&self) -> u32 {
        self.0.atom_count
    }

    /// FNV-1a fingerprint over source id and active coordinate bits.
    #[getter]
    fn coordinate_hash(&self) -> u64 {
        self.0.coordinate_hash
    }

    /// Column-major model-to-world transform.
    #[getter]
    fn model_to_world(&self) -> [f32; 16] {
        self.0.model_to_world
    }

    /// Caller- or provider-supplied residue classifications.
    #[getter]
    fn secondary_structure(&self) -> Vec<String> {
        self.0.secondary_structure.clone()
    }
}

/// One stored selection expanded into stable structure-local atom rows.
#[pyclass(name = "SelectionDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PySelectionDescription(pub(crate) molgfx::core::SelectionDescription);

#[pymethods]
impl PySelectionDescription {
    /// Stable selection slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Generation of the selection handle.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// Per-structure rows, sorted by structure identity.
    #[getter]
    fn masks(&self) -> Vec<PySelectionMask> {
        self.0.masks.iter().cloned().map(PySelectionMask).collect()
    }
}

/// Counts of dense or caller-owned scene tables.
#[pyclass(name = "TableCounts", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyTableCounts(pub(crate) molgfx::core::TableCounts);

#[pymethods]
impl PyTableCounts {
    /// Interaction edges.
    #[getter]
    fn interactions(&self) -> u64 {
        self.0.interactions
    }

    /// Analytic guides.
    #[getter]
    fn guides(&self) -> u64 {
        self.0.guides
    }

    /// Notes and markers.
    #[getter]
    fn annotations(&self) -> u64 {
        self.0.annotations
    }

    /// Caller-computed measurements.
    #[getter]
    fn measurements(&self) -> u64 {
        self.0.measurements
    }

    /// Per-atom scalar columns.
    #[getter]
    fn atom_properties(&self) -> u64 {
        self.0.atom_properties
    }

    /// Shared-mesh instances.
    #[getter]
    fn mesh_instances(&self) -> u64 {
        self.0.mesh_instances
    }

    /// Screen overlays.
    #[getter]
    fn overlays(&self) -> u64 {
        self.0.overlays
    }

    /// Compact ligand pose batches.
    #[getter]
    fn ligand_pose_batches(&self) -> u64 {
        self.0.ligand_pose_batches
    }

    /// Generic point batches.
    #[getter]
    fn point_batches(&self) -> u64 {
        self.0.point_batches
    }

    /// Shared-template instance batches.
    #[getter]
    fn instance_batches(&self) -> u64 {
        self.0.instance_batches
    }

    /// Generic typed attribute columns.
    #[getter]
    fn attributes(&self) -> u64 {
        self.0.attributes
    }

    /// Generic relation batches.
    #[getter]
    fn relation_batches(&self) -> u64 {
        self.0.relation_batches
    }

    /// Generic domain visual descriptors.
    #[getter]
    fn domain_visuals(&self) -> u64 {
        self.0.domain_visuals
    }
}
