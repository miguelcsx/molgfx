//! Prepared atomic updates for the live semantic and physical scene.
//!
//! A patch is prepared in full — every query evaluated, every value validated,
//! every physical record computed — against the unchanged scene, and only then
//! committed. A patch that changes which representations or scientific items
//! exist re-resolves the scene; everything else, including recolouring,
//! retargeting a representation and editing appearance rules, is applied in
//! place to the representations and columns it touches.

use crate::error::{Error, PatchError};
use crate::id::{RepresentationId, StructureId};
use crate::representation::form::RepresentationSpec;
use crate::scene::Resolution;
use crate::scene::runtime::candidate_spec;
use crate::scene::selection_rows::SelectionRows;
use crate::spec::{PatchOperation, ScenePatch, SceneSpec};
use crate::{
    AnnotationId, AnnotationSpec, MeasurementId, MeasurementSpec, ScientificInteractionId,
    ScientificInteractionSpec, TrajectoryId, TrajectorySpec, VolumeId, VolumeSpec,
};
use interactions::InteractionUpdates;
use molgfx_core::{Representation, RepresentationHandle, StructureHandle};
use routing::is_structural;
use std::collections::{BTreeMap, BTreeSet};

mod appearance;
mod commit;
mod interactions;
mod lower;
mod routing;
mod science;
mod targets;

pub(crate) use commit::CommitTarget;

pub(crate) enum PatchPlan {
    Structural {
        spec: Box<SceneSpec>,
        resolution: Box<Resolution>,
    },
    Local(Box<LocalPatchPlan>),
}

#[derive(Clone, Copy)]
pub(crate) struct PatchInputs<'a> {
    pub(crate) spec: &'a SceneSpec,
    pub(crate) scene: &'a molgfx_core::Scene,
    pub(crate) handles: &'a BTreeMap<RepresentationId, RepresentationHandle>,
    pub(crate) visuals: &'a BTreeMap<RepresentationId, crate::visual::ResolvedVisual>,
    pub(crate) properties: &'a BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
    pub(crate) structures: &'a BTreeMap<StructureId, molgfx_core::MolecularSource>,
    pub(crate) property_bindings: &'a BTreeMap<Box<str>, crate::ScalarPropertyBinding>,
    pub(crate) science_bindings: &'a crate::science::ScienceBindings,
    pub(crate) structure_assets: &'a crate::scene::runtime::StructureAssets,
    pub(crate) rows: &'a SelectionRows,
}

impl PatchInputs<'_> {
    /// The core handle of every bound structure, keyed by semantic identity.
    fn structure_handles(&self) -> BTreeMap<StructureId, StructureHandle> {
        self.structures
            .keys()
            .copied()
            .zip(self.scene.structures().map(|(handle, _)| handle))
            .collect()
    }
}

pub(crate) struct LocalPatchPlan {
    representations: BTreeMap<RepresentationId, RepresentationSpec>,
    physical: Vec<(RepresentationId, RepresentationHandle, Representation)>,
    visual_updates: BTreeMap<RepresentationId, Option<crate::visual::ResolvedVisual>>,
    visual_replacements: BTreeSet<RepresentationId>,
    parameter_updates: BTreeMap<RepresentationId, BTreeSet<Box<str>>>,
    color_updates: BTreeSet<RepresentationId>,
    targets: targets::TargetUpdates,
    appearance: appearance::AppearanceUpdates,
    interactions: InteractionUpdates,
    science: ScienceDomains,
    camera: Change<molgfx_math::Camera>,
    /// Whether the patch carried any operation at all.
    ///
    /// Only the emptiness is ever read, so the plan records the answer rather
    /// than owning a second copy of the operations: an edit clones the patch's
    /// operation list purely to ask whether it was empty.
    touched: bool,
}

#[derive(Default)]
struct ScienceDomains {
    volumes: Option<BTreeMap<VolumeId, VolumeSpec>>,
    annotations: Option<BTreeMap<AnnotationId, AnnotationSpec>>,
    measurements: Option<BTreeMap<MeasurementId, MeasurementSpec>>,
    interactions: Option<BTreeMap<ScientificInteractionId, ScientificInteractionSpec>>,
    trajectories: Option<BTreeMap<TrajectoryId, TrajectorySpec>>,
}

#[derive(Clone, Default)]
pub(super) enum Change<T> {
    #[default]
    Unchanged,
    Set(Option<T>),
}

impl PatchPlan {
    pub(crate) fn prepare(inputs: PatchInputs<'_>, patch: &ScenePatch) -> Result<Self, Error> {
        if patch.operations.iter().any(is_structural) {
            let candidate = candidate_spec(inputs.spec, patch)?;
            // A structural patch in this planner only ever adds, removes or
            // replaces representations and scientific items; the molecules are
            // untouched, so their atom tables are reused rather than rebuilt,
            // and unchanged queries come from the evaluated-rows cache.
            let resolution = crate::scene::runtime::resolve_reusing(
                &candidate,
                inputs.structures,
                inputs.property_bindings,
                inputs.science_bindings,
                inputs.rows,
                Some(inputs.structure_assets),
            )
            .map_err(|error| Error::InvalidSpec(format!("patch could not be resolved: {error}")))?;
            return Ok(Self::Structural {
                spec: Box::new(candidate),
                resolution: Box::new(resolution),
            });
        }
        Ok(Self::Local(Box::new(LocalPatchPlan::prepare(
            inputs, patch,
        )?)))
    }
}

impl LocalPatchPlan {
    fn prepare(inputs: PatchInputs<'_>, patch: &ScenePatch) -> Result<Self, Error> {
        let mut plan = Self {
            representations: BTreeMap::new(),
            physical: Vec::new(),
            visual_updates: BTreeMap::new(),
            visual_replacements: BTreeSet::new(),
            parameter_updates: BTreeMap::new(),
            color_updates: BTreeSet::new(),
            targets: targets::TargetUpdates::default(),
            appearance: appearance::AppearanceUpdates::default(),
            interactions: InteractionUpdates::default(),
            science: ScienceDomains::default(),
            camera: Change::Unchanged,
            touched: !patch.operations.is_empty(),
        };
        for operation in &patch.operations {
            plan.apply_semantic_operation(inputs.spec, operation)?;
        }
        plan.validate_and_lower(inputs)?;
        Ok(plan)
    }

    fn apply_semantic_operation(
        &mut self,
        spec: &SceneSpec,
        operation: &PatchOperation,
    ) -> Result<(), Error> {
        if self.science.apply(spec, operation)? || self.appearance.apply(spec, operation)? {
            return Ok(());
        }
        match operation {
            PatchOperation::SetVisibility { id, visible } => {
                self.representation(spec, *id)?.common.visible = *visible;
            }
            PatchOperation::SetOpacity { id, opacity } => {
                if !opacity.is_finite() || !(0.0..=1.0).contains(opacity) {
                    return Err(PatchError::Invalid(
                        "opacity must be finite and between zero and one".to_owned(),
                    )
                    .into());
                }
                self.representation(spec, *id)?.common.opacity = *opacity;
            }
            PatchOperation::SetColor { id, color } => {
                color.validate()?;
                self.representation(spec, *id)?.common.color = color.clone();
                let _ = self.color_updates.insert(*id);
            }
            PatchOperation::SetRepresentationTarget { id, target } => {
                let _ = target.fingerprint()?;
                self.representation(spec, *id)?.common.target = target.clone();
                self.targets.mark(*id);
            }
            PatchOperation::SetVisual { id, visual } => {
                let representation = self.representation(spec, *id)?;
                representation.common.visual.clone_from(visual);
                representation.common.parameters.clear();
                let _ = self.visual_replacements.insert(*id);
            }
            PatchOperation::SetParameter { id, name, value } => {
                let representation = self.representation(spec, *id)?;
                if let Some(value) = value {
                    let _ = representation
                        .common
                        .parameters
                        .insert(name.clone(), value.clone());
                } else {
                    let _ = representation.common.parameters.remove(name);
                }
                let _ = self
                    .parameter_updates
                    .entry(*id)
                    .or_default()
                    .insert(name.clone());
            }
            PatchOperation::SetFocus { selection } => {
                self.interactions.focus = Change::Set(selection.clone());
            }
            PatchOperation::SetInteraction { channel, selection } => {
                self.interactions.set(channel, selection.clone());
            }
            PatchOperation::SetCamera { camera } => self.camera = Change::Set(*camera),
            PatchOperation::AddStructure { .. }
            | PatchOperation::AddRepresentation { .. }
            | PatchOperation::RemoveRepresentation { .. }
            | PatchOperation::ReplaceRepresentation { .. } => {
                return Err(Error::InvalidSpec(
                    "structural operation reached the local patch planner".to_owned(),
                ));
            }
            PatchOperation::AddVolume { .. }
            | PatchOperation::RemoveVolume { .. }
            | PatchOperation::AddAnnotation { .. }
            | PatchOperation::RemoveAnnotation { .. }
            | PatchOperation::AddMeasurement { .. }
            | PatchOperation::RemoveMeasurement { .. }
            | PatchOperation::AddScientificInteraction { .. }
            | PatchOperation::RemoveScientificInteraction { .. }
            | PatchOperation::AddTrajectory { .. }
            | PatchOperation::RemoveTrajectory { .. }
            | PatchOperation::AddAppearanceRule { .. }
            | PatchOperation::ReplaceAppearanceRule { .. }
            | PatchOperation::RemoveAppearanceRule { .. } => {
                return Err(Error::InvalidSpec(
                    "domain operation was not prepared".to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn representation(
        &mut self,
        spec: &SceneSpec,
        id: RepresentationId,
    ) -> Result<&mut RepresentationSpec, Error> {
        if let std::collections::btree_map::Entry::Vacant(entry) = self.representations.entry(id) {
            let representation = spec
                .representations
                .get(&id)
                .cloned()
                .ok_or(PatchError::MissingId)?;
            let _ = entry.insert(representation);
        }
        self.representations.get_mut(&id).ok_or(Error::MissingId)
    }
}

impl<T> Change<T> {
    fn value(&self) -> Option<&T> {
        match self {
            Self::Set(Some(value)) => Some(value),
            Self::Unchanged | Self::Set(None) => None,
        }
    }
}

pub(super) fn assign<T>(target: &mut Option<T>, update: Change<T>) {
    if let Change::Set(update) = update {
        *target = update;
    }
}
