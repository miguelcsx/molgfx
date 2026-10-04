//! A patch that is applied in place to the live scene.

use super::{Change, OverlayDomains, PatchInputs, appearance, interactions, targets};
use crate::error::{Error, PatchError};
use crate::id::RepresentationId;
use crate::representation::form::RepresentationSpec;
use crate::spec::{PatchOperation, ScenePatch, SceneSpec};
use interactions::InteractionUpdates;
use molgfx_core::{Representation, RepresentationHandle};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct LocalPatchPlan {
    pub(super) representations: BTreeMap<RepresentationId, RepresentationSpec>,
    pub(super) physical: Vec<(RepresentationId, RepresentationHandle, Representation)>,
    pub(super) segmentation_physical: Vec<(RepresentationHandle, Representation)>,
    pub(super) visual_updates: BTreeMap<RepresentationId, Option<crate::visual::ResolvedVisual>>,
    pub(super) visual_replacements: BTreeSet<RepresentationId>,
    pub(super) parameter_updates: BTreeMap<RepresentationId, BTreeSet<Box<str>>>,
    pub(super) color_updates: BTreeSet<RepresentationId>,
    pub(super) targets: targets::TargetUpdates,
    pub(super) appearance: appearance::AppearanceUpdates,
    pub(super) interactions: InteractionUpdates,
    pub(super) overlay: OverlayDomains,
    pub(super) camera: Change<molgfx_math::Camera>,
    /// Whether the patch carried any operation at all.
    ///
    /// Only the emptiness is ever read, so the plan records the answer rather
    /// than owning a second copy of the operations: an edit clones the patch's
    /// operation list purely to ask whether it was empty.
    pub(super) touched: bool,
}

impl LocalPatchPlan {
    pub(super) fn prepare(inputs: PatchInputs<'_>, patch: &ScenePatch) -> Result<Self, Error> {
        let mut plan = Self {
            representations: BTreeMap::new(),
            physical: Vec::new(),
            segmentation_physical: Vec::new(),
            visual_updates: BTreeMap::new(),
            visual_replacements: BTreeSet::new(),
            parameter_updates: BTreeMap::new(),
            color_updates: BTreeSet::new(),
            targets: targets::TargetUpdates::default(),
            appearance: appearance::AppearanceUpdates::default(),
            interactions: InteractionUpdates::default(),
            overlay: OverlayDomains::default(),
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
        if self.overlay.apply(spec, operation)? || self.appearance.apply(spec, operation)? {
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
            | PatchOperation::AddSegmentation { .. }
            | PatchOperation::RemoveSegmentation { .. }
            | PatchOperation::SetSegmentStyles { .. }
            | PatchOperation::SetVolumeIsovalue { .. }
            | PatchOperation::AddAnnotation { .. }
            | PatchOperation::RemoveVolume { .. }
            | PatchOperation::RemoveAnnotation { .. }
            | PatchOperation::AddMeasurement { .. }
            | PatchOperation::RemoveMeasurement { .. }
            | PatchOperation::AddInteraction { .. }
            | PatchOperation::RemoveInteraction { .. }
            | PatchOperation::AddTrajectory { .. }
            | PatchOperation::RemoveTrajectory { .. }
            | PatchOperation::AddEllipsoids { .. }
            | PatchOperation::RemoveEllipsoids { .. }
            | PatchOperation::AddPlane { .. }
            | PatchOperation::RemovePlane { .. }
            | PatchOperation::AddAppearanceRule { .. }
            | PatchOperation::ReplaceAppearanceRule { .. }
            | PatchOperation::SetAssembly { .. }
            | PatchOperation::SetFitting { .. }
            | PatchOperation::SetValidation { .. }
            | PatchOperation::RestoreSnapshot(_)
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
