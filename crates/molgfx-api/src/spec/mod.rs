//! Canonical scene specifications and atomic patches.

pub(crate) mod lowering;

use crate::id::{RepresentationId, StructureId};
pub(crate) use crate::patch::{PatchOperation, ScenePatch};
use crate::representation::Selection;
use crate::representation::form::RepresentationSpec;
use crate::science::{
    AnnotationSpec, MeasurementSpec, ScientificInteractionSpec, TrajectorySpec, VolumeSpec,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// GPU-resident semantic interaction channel.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionChannel {
    /// User selection.
    Selected,
    /// Current pointer hover.
    Hovered,
    /// Focus target.
    Focused,
    /// De-emphasized context.
    Muted,
    /// Explicitly hidden entities.
    Hidden,
    /// Namespaced application-defined state bit.
    Custom(Box<str>),
}

/// Portable description of where molecular data can be resolved.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct StructureSource {
    /// SHA-256 identity of the bound coordinate snapshot.
    pub content_hash: Box<str>,
    /// Optional portable origin.
    pub uri: Option<Box<str>>,
    /// Optional format hint.
    pub format: Option<Box<str>>,
}

/// Immutable renderer-independent scene state.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SceneSpec {
    /// Revision used by incremental patches.
    pub revision: u64,
    /// Molecular inputs in stable ID order.
    pub structures: BTreeMap<StructureId, StructureSource>,
    /// Scalar-property descriptors; value columns are runtime bindings.
    #[serde(default)]
    pub properties: BTreeMap<Box<str>, crate::PropertySpec>,
    /// Representations in stable ID order.
    pub representations: BTreeMap<RepresentationId, RepresentationSpec>,
    /// Density-volume specifications in stable ID order.
    #[serde(default)]
    pub volumes: BTreeMap<crate::VolumeId, VolumeSpec>,
    /// Annotation specifications in stable ID order.
    #[serde(default)]
    pub annotations: BTreeMap<crate::AnnotationId, AnnotationSpec>,
    /// Measurement specifications in stable ID order.
    #[serde(default)]
    pub measurements: BTreeMap<crate::MeasurementId, MeasurementSpec>,
    /// Scientific interactions in stable ID order.
    #[serde(default)]
    pub scientific_interactions:
        BTreeMap<crate::ScientificInteractionId, ScientificInteractionSpec>,
    /// Trajectory bindings in stable ID order.
    #[serde(default)]
    pub trajectories: BTreeMap<crate::TrajectoryId, TrajectorySpec>,
    /// Optional focus selection.
    pub focus: Option<Selection>,
    /// Selected entity query.
    pub selected: Option<Selection>,
    /// Hovered entity query.
    pub hovered: Option<Selection>,
    /// Muted entity query.
    pub muted: Option<Selection>,
    /// Hidden entity query.
    pub hidden: Option<Selection>,
    /// Namespaced custom interaction channels.
    #[serde(default)]
    pub custom_interactions: BTreeMap<Box<str>, Selection>,
    /// Optional explicit camera; render targets update only its aspect ratio.
    #[serde(default)]
    pub camera: Option<molgfx_math::Camera>,
    /// Namespaced extension values preserved across round trips.
    pub extensions: BTreeMap<Box<str>, serde_json::Value>,
}

impl SceneSpec {
    pub(crate) fn empty() -> Self {
        Self {
            revision: 0,
            structures: BTreeMap::new(),
            properties: BTreeMap::new(),
            representations: BTreeMap::new(),
            volumes: BTreeMap::new(),
            annotations: BTreeMap::new(),
            measurements: BTreeMap::new(),
            scientific_interactions: BTreeMap::new(),
            trajectories: BTreeMap::new(),
            focus: None,
            selected: None,
            hovered: None,
            muted: None,
            hidden: None,
            custom_interactions: BTreeMap::new(),
            camera: None,
            extensions: BTreeMap::new(),
        }
    }

    /// Deterministic JSON encoding.
    ///
    /// # Errors
    ///
    /// Returns an error if the specification cannot be serialized.
    pub fn to_json(&self) -> Result<String, crate::Error> {
        serde_json::to_string(self).map_err(crate::Error::from)
    }

    /// Reads a canonical scene specification.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed JSON or invalid values.
    pub fn from_json(source: &str) -> Result<Self, crate::Error> {
        let value: Self = serde_json::from_str(source)?;
        value.validate_selections()?;
        value.validate_science()?;
        value.validate_camera()?;
        Ok(value)
    }

    /// Stable content hash of the canonical serialized semantic state.
    #[must_use]
    pub fn stable_hash(&self) -> String {
        stable_json_hash(self)
    }

    /// Applies a patch to this immutable value and returns the new value.
    ///
    /// # Errors
    ///
    /// Returns a revision conflict or validation error without changing `self`.
    pub fn patched(&self, patch: &ScenePatch) -> Result<Self, crate::Error> {
        if patch.base_revision != self.revision {
            return Err(crate::PatchError::RevisionConflict {
                expected: patch.base_revision,
                actual: self.revision,
            }
            .into());
        }
        crate::scene::runtime::candidate_spec(self, patch)
    }

    pub(crate) fn validate_selections(&self) -> Result<(), crate::Error> {
        for selection in [
            self.focus.as_ref(),
            self.selected.as_ref(),
            self.hovered.as_ref(),
            self.muted.as_ref(),
            self.hidden.as_ref(),
        ]
        .into_iter()
        .flatten()
        .chain(self.custom_interactions.values())
        {
            let _ = selection.stable_hash()?;
        }
        for representation in self.representations.values() {
            representation.validate()?;
            let _ = representation.common.target.stable_hash()?;
            let Some(structure) = representation.common.structure else {
                return Err(crate::Error::InvalidSpec(
                    "every representation must target a structure".to_owned(),
                ));
            };
            if !self.structures.contains_key(&structure) {
                return Err(crate::Error::InvalidSpec(
                    "representation targets an unknown structure".to_owned(),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_camera(&self) -> Result<(), crate::Error> {
        validate_camera(self.camera)
    }

    pub(crate) fn validate_science(&self) -> Result<(), crate::Error> {
        self.volumes.values().try_for_each(VolumeSpec::validate)?;
        self.annotations
            .values()
            .try_for_each(|spec| spec.validate(self))?;
        self.measurements
            .values()
            .try_for_each(|spec| spec.validate(self))?;
        self.scientific_interactions
            .values()
            .try_for_each(|spec| spec.validate(self))?;
        self.trajectories
            .values()
            .try_for_each(|spec| spec.validate(self))
    }
}

pub(crate) fn validate_camera(camera: Option<molgfx_math::Camera>) -> Result<(), crate::Error> {
    let Some(camera) = camera else {
        return Ok(());
    };
    let vectors_are_finite = [camera.eye, camera.target, camera.up]
        .iter()
        .all(|value| value.is_finite());
    if !vectors_are_finite
        || camera.eye == camera.target
        || camera.up.length_squared() <= f32::EPSILON
        || !camera.projection.aspect().is_finite()
        || camera.projection.aspect() <= 0.0
    {
        return Err(crate::Error::InvalidSpec(
            "camera vectors and projection must be finite and non-degenerate".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn stable_json_hash(value: &impl Serialize) -> String {
    use sha2::Digest as _;
    let bytes = match serde_json::to_vec(value) {
        Ok(bytes) => bytes,
        Err(error) => error.to_string().into_bytes(),
    };
    format!("{:x}", sha2::Sha256::digest(bytes))
}
