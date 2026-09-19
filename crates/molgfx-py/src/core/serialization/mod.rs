//! Python adapters for the process-independent scene-manifest records.
//!
//! A manifest is a description, not a scene. It names rows, generations and
//! fingerprints so a second process can rebuild the same composition without
//! sharing a handle, and it keeps caller-owned data — coordinates, voxels,
//! property values — outside itself behind content addresses.
//!
//! That is the rule these adapters follow: **a record the engine produces is
//! read-only, and a record a caller must hand back gets a constructor.** So
//! every description is exposed with getters, while the identity vocabulary
//! (`ObjectIdentity`, `ContentAddress`, `PayloadReference`) also carries
//! constructors, because a caller assembles those when declaring payloads.
//!
//! Numeric knobs the manifest keeps as flat arrays — `params`, `transform`,
//! `planes`, ramp stops — are exposed as tuples rather than invented named
//! fields, so the Python view keeps the manifest's own shape and a schema
//! change shows up in one place.

mod composition;
mod display;
mod generic;
mod identity;
mod manifest;
mod representation;
mod volume;

pub(crate) use composition::{
    PySceneDescription, PySelectionDescription, PyStructureDescription, PyTableCounts,
};
pub(crate) use display::{
    PyAnnotationDescription, PyGuideDescription, PyGuideStyleDescription, PyInteractionDescription,
    PyLigandPoseBatchDescription, PyLigandPoseDescription, PyMarkerStyleDescription,
    PyMeasurementDescription,
};
pub(crate) use generic::{
    PyAttributeDescription, PyDomainVisualDescription, PyInstanceBatchDescription,
    PyPointBatchDescription, PyRelationBatchDescription,
};
pub(crate) use identity::{
    PyAnchorDescription, PyEntityDescription, PyObjectIdentity, PyRowDomainDescription,
    PySelectionMask, PySourceRowsDescription, PyTargetDescription,
};
pub(crate) use manifest::{
    PyContentAddress, PyPayloadReference, PyReferencedPayloadKind, PySceneManifest, read_manifest,
    write_manifest,
};
pub(crate) use representation::{
    PyClipDescription, PyColorDescription, PyMaterialDescription, PyRepresentationDescription,
    PySurfaceComponentDescription, PyVisualAttributeDescription, PyVisualInstructionDescription,
    PyVisualStyleDescription,
};
pub(crate) use volume::{
    PyAtomPropertyDescription, PyOccupancyDescription, PyPropertyAppearanceDescription,
    PyRegionDescription, PyScalarSemanticsDescription, PySegmentStyleDescription,
    PySegmentationStyleDescription, PySurfaceScalarDescription, PyVolumeDescription,
    PyVolumeStyleDescription, PyVolumeTransferPointDescription,
};
