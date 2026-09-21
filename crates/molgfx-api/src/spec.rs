//! Canonical scene specifications and atomic patches.

use crate::color::ColorSpec;
use crate::id::{RepresentationId, StructureId};
pub(crate) use crate::patch::{PatchOperation, ScenePatch};
use crate::representation::{CartoonStyle, Selection, SurfaceKind, SurfaceStyle};
use crate::visual::{ParameterValue, VisualStyle};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Fine-grained semantic revisions used by physical cache keys.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Revisions {
    /// Molecular topology changed.
    pub topology: u64,
    /// Coordinate storage changed.
    pub coordinates: u64,
    /// A canonical selection changed.
    pub selection: u64,
    /// Color, opacity or visual style changed.
    pub appearance: u64,
    /// Selected, hovered, focused, muted or hidden state changed.
    pub interaction: u64,
    /// An assembly or symmetry placement changed.
    pub placement: u64,
    /// Volume brick data changed.
    pub volume_bricks: u64,
    /// Custom mesh attributes changed.
    pub mesh_attributes: u64,
    /// Camera and renderer-independent view state changed.
    pub view: u64,
}

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

/// Internal representation form serialized by semantic name, never GPU tag.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RepresentationForm {
    Cartoon,
    BallAndStick,
    Spacefill,
    Licorice,
    Lines,
    Points,
    Surface,
    NucleicAcid,
    Bases,
    BasePairs,
    Glycan,
}

/// Immutable serializable representation specification.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RepresentationSpec {
    pub(crate) structure: Option<StructureId>,
    pub(crate) target: Selection,
    pub(crate) form: RepresentationForm,
    pub(crate) color: ColorSpec,
    pub(crate) opacity: f32,
    pub(crate) radius: Option<f32>,
    pub(crate) bond_radius: Option<f32>,
    pub(crate) width: Option<f32>,
    pub(crate) probe_radius: Option<f32>,
    pub(crate) isolevel: Option<f32>,
    pub(crate) cartoon_style: Option<CartoonStyle>,
    pub(crate) surface_kind: Option<SurfaceKind>,
    pub(crate) surface_style: Option<SurfaceStyle>,
    #[serde(default)]
    pub(crate) visual: Option<VisualStyle>,
    #[serde(default)]
    pub(crate) parameters: BTreeMap<Box<str>, ParameterValue>,
    pub(crate) visible: bool,
}

impl RepresentationSpec {
    pub(crate) fn new(target: Selection, form: RepresentationForm) -> Self {
        Self {
            structure: None,
            target,
            form,
            color: ColorSpec::default(),
            opacity: 1.0,
            radius: None,
            bond_radius: None,
            width: None,
            probe_radius: None,
            isolevel: None,
            cartoon_style: None,
            surface_kind: None,
            surface_style: None,
            visual: None,
            parameters: BTreeMap::new(),
            visible: true,
        }
    }

    /// `MolFrame` query selecting the represented atoms.
    #[must_use]
    pub fn selection(&self) -> &str {
        self.target.source()
    }

    /// Molecular asset this representation targets after scene insertion.
    #[must_use]
    pub const fn structure_id(&self) -> Option<StructureId> {
        self.structure
    }

    /// Effective opacity.
    #[must_use]
    pub const fn opacity(&self) -> f32 {
        self.opacity
    }

    /// Stable content hash used by semantic and shader caches.
    #[must_use]
    pub fn stable_hash(&self) -> String {
        stable_json_hash(self)
    }

    /// Deterministic representation explanation.
    #[must_use]
    pub fn explain(&self) -> String {
        format!(
            "Representation\nform: {:?}\nselection: {}\nhash: {}",
            self.form,
            self.selection(),
            self.stable_hash()
        )
    }

    pub(crate) fn validate(&self) -> Result<(), crate::Error> {
        self.color.validate()?;
        if let Some(visual) = &self.visual {
            let _ = visual.compile()?;
            let _ = crate::visual_native::lower(visual, &self.parameters)?;
        } else if !self.parameters.is_empty() {
            return Err(crate::Error::InvalidSpec(
                "visual parameter values require a visual style".to_owned(),
            ));
        }
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err(crate::Error::InvalidSpec(
                "opacity must be finite and between zero and one".to_owned(),
            ));
        }
        for (name, value) in [
            ("radius", self.radius),
            ("bond radius", self.bond_radius),
            ("width", self.width),
            ("probe radius", self.probe_radius),
        ] {
            if value.is_some_and(|number| !number.is_finite() || number <= 0.0) {
                return Err(crate::Error::InvalidSpec(format!(
                    "{name} must be finite and positive"
                )));
            }
        }
        if self.isolevel.is_some_and(|number| !number.is_finite()) {
            return Err(crate::Error::InvalidSpec(
                "isolevel must be finite".to_owned(),
            ));
        }
        if self.form != RepresentationForm::Surface
            && (self.probe_radius.is_some()
                || self.isolevel.is_some()
                || self.surface_kind.is_some()
                || self.surface_style.is_some())
        {
            return Err(crate::Error::InvalidSpec(
                "surface controls require a surface representation".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Immutable renderer-independent scene state.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SceneSpec {
    /// Wire format version.
    pub version: u32,
    /// Revision used by incremental patches.
    pub revision: u64,
    /// Independent cache-invalidation revisions.
    #[serde(default)]
    pub revisions: Revisions,
    /// Molecular inputs in stable ID order.
    pub structures: BTreeMap<StructureId, StructureSource>,
    /// Representations in stable ID order.
    pub representations: BTreeMap<RepresentationId, RepresentationSpec>,
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
            version: 1,
            revision: 0,
            revisions: Revisions::default(),
            structures: BTreeMap::new(),
            representations: BTreeMap::new(),
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
    /// Returns an error for malformed JSON, unsupported versions, or invalid values.
    pub fn from_json(source: &str) -> Result<Self, crate::Error> {
        let value: Self = serde_json::from_str(source)?;
        if value.version != 1 {
            return Err(crate::Error::InvalidSpec(format!(
                "unsupported SceneSpec version {}",
                value.version
            )));
        }
        for representation in value.representations.values() {
            representation.validate()?;
        }
        value.validate_selections()?;
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
        crate::scene_runtime::candidate_spec(self, patch)
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
            let Some(structure) = representation.structure else {
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
        let Some(camera) = self.camera else {
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
}

fn stable_json_hash(value: &impl Serialize) -> String {
    use sha2::Digest as _;
    let bytes = match serde_json::to_vec(value) {
        Ok(bytes) => bytes,
        Err(error) => error.to_string().into_bytes(),
    };
    format!("{:x}", sha2::Sha256::digest(bytes))
}
