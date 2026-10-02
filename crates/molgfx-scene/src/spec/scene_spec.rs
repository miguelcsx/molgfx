//! The canonical declarative description of a scene.

use super::{ScenePatch, StructureSource, stable_json_hash, validate_camera};
use crate::id::{RepresentationId, StructureId};
use crate::overlay::{
    AnnotationSpec, AssemblySpec, InteractionSpec, MeasurementSpec, TrajectorySpec, VolumeSpec,
};
use crate::representation::Selection;
use crate::representation::form::RepresentationSpec;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    /// Interactions in stable ID order.
    #[serde(default)]
    pub interactions: BTreeMap<crate::InteractionId, InteractionSpec>,
    /// Trajectory bindings in stable ID order.
    #[serde(default)]
    pub trajectories: BTreeMap<crate::TrajectoryId, TrajectorySpec>,
    /// Per-atom anisotropic-displacement ellipsoid overlays in stable ID order.
    #[serde(default)]
    pub ellipsoids: BTreeMap<crate::EllipsoidId, crate::overlay::EllipsoidSpec>,
    /// Caller-authored planar guide regions in stable ID order.
    #[serde(default)]
    pub planes: BTreeMap<crate::PlaneId, crate::PlaneSpec>,
    /// Optional molecular assembly and crystallographic unit-cell description.
    #[serde(default)]
    pub assembly: Option<AssemblySpec>,
    /// Selection-scoped colour rules. Where rules overlap, the higher identity
    /// wins; a rule always wins over a representation's own colour.
    #[serde(default)]
    pub appearance: BTreeMap<crate::AppearanceRuleId, crate::AppearanceRuleSpec>,
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
            interactions: BTreeMap::new(),
            trajectories: BTreeMap::new(),
            ellipsoids: BTreeMap::new(),
            planes: BTreeMap::new(),
            assembly: None,
            appearance: BTreeMap::new(),
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
        value.validate_overlay()?;
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
        crate::scene::apply::candidate_spec(self, patch)
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
        for rule in self.appearance.values() {
            crate::patch::appearance_ops::validate(self, rule)?;
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

    pub(crate) fn validate_overlay(&self) -> Result<(), crate::Error> {
        if let Some(assembly) = &self.assembly {
            assembly.validate()?;
        }
        self.volumes.values().try_for_each(VolumeSpec::validate)?;
        self.annotations
            .values()
            .try_for_each(|spec| spec.validate(self))?;
        self.measurements
            .values()
            .try_for_each(|spec| spec.validate(self))?;
        self.interactions
            .values()
            .try_for_each(|spec| spec.validate(self))?;
        self.trajectories
            .values()
            .try_for_each(|spec| spec.validate(self))?;
        self.ellipsoids
            .values()
            .try_for_each(|spec| spec.validate(self))?;
        self.planes
            .values()
            .try_for_each(|spec| spec.validate(self))
    }
}
