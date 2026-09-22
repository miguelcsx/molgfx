//! Prepared atomic updates for the live semantic and physical scene.

use crate::error::{Error, PatchError};
use crate::id::{RepresentationId, StructureId};
use crate::representation::Selection;
use crate::representation::form::RepresentationSpec;
use crate::scene::Resolution;
use crate::scene::runtime::{advance_domain_revisions, candidate_spec, resolve};
use crate::spec::{InteractionChannel, PatchOperation, ScenePatch, SceneSpec};
use crate::{
    AnnotationId, AnnotationSpec, MeasurementId, MeasurementSpec, ScientificInteractionId,
    ScientificInteractionSpec, TrajectoryId, TrajectorySpec, VolumeId, VolumeSpec,
};
use molgfx_core::{Representation, RepresentationHandle};
use std::collections::{BTreeMap, BTreeSet};

mod science;

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
}

pub(crate) struct LocalPatchPlan {
    representations: BTreeMap<RepresentationId, RepresentationSpec>,
    physical: Vec<(RepresentationHandle, Representation)>,
    visual_updates: BTreeMap<RepresentationId, Option<crate::visual::ResolvedVisual>>,
    visual_replacements: BTreeSet<RepresentationId>,
    parameter_updates: BTreeMap<RepresentationId, BTreeSet<Box<str>>>,
    interactions: InteractionUpdates,
    science: ScienceDomains,
    camera: Change<molgfx_math::Camera>,
    operations: Vec<PatchOperation>,
}

struct InteractionUpdates {
    focus: Change<Selection>,
    selected: Change<Selection>,
    hovered: Change<Selection>,
    muted: Change<Selection>,
    hidden: Change<Selection>,
    custom: BTreeMap<Box<str>, Option<Selection>>,
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
enum Change<T> {
    #[default]
    Unchanged,
    Set(Option<T>),
}

impl Default for InteractionUpdates {
    fn default() -> Self {
        Self {
            focus: Change::Unchanged,
            selected: Change::Unchanged,
            hovered: Change::Unchanged,
            muted: Change::Unchanged,
            hidden: Change::Unchanged,
            custom: BTreeMap::new(),
        }
    }
}

impl PatchPlan {
    pub(crate) fn prepare(inputs: PatchInputs<'_>, patch: &ScenePatch) -> Result<Self, Error> {
        if patch.operations.iter().any(is_structural) {
            let candidate = candidate_spec(inputs.spec, patch)?;
            let resolution = resolve(&candidate, inputs.structures, inputs.property_bindings)
                .map_err(|error| {
                    Error::InvalidSpec(format!("patch could not be resolved: {error}"))
                })?;
            return Ok(Self::Structural {
                spec: Box::new(candidate),
                resolution: Box::new(resolution),
            });
        }
        Ok(Self::Local(Box::new(LocalPatchPlan::prepare(
            inputs.spec,
            inputs.scene,
            inputs.handles,
            inputs.visuals,
            inputs.properties,
            patch,
        )?)))
    }
}

impl LocalPatchPlan {
    fn prepare(
        spec: &SceneSpec,
        scene: &molgfx_core::Scene,
        handles: &BTreeMap<RepresentationId, RepresentationHandle>,
        visuals: &BTreeMap<RepresentationId, crate::visual::ResolvedVisual>,
        properties: &BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
        patch: &ScenePatch,
    ) -> Result<Self, Error> {
        let mut plan = Self {
            representations: BTreeMap::new(),
            physical: Vec::new(),
            visual_updates: BTreeMap::new(),
            visual_replacements: BTreeSet::new(),
            parameter_updates: BTreeMap::new(),
            interactions: InteractionUpdates::default(),
            science: ScienceDomains::default(),
            camera: Change::Unchanged,
            operations: patch.operations.clone(),
        };
        for operation in &patch.operations {
            plan.apply_semantic_operation(spec, operation)?;
        }
        plan.validate_and_lower(spec, scene, handles, visuals, properties)?;
        Ok(plan)
    }

    fn apply_semantic_operation(
        &mut self,
        spec: &SceneSpec,
        operation: &PatchOperation,
    ) -> Result<(), Error> {
        if self.science.apply(spec, operation)? {
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
            PatchOperation::AddRepresentation { .. }
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
            | PatchOperation::RemoveTrajectory { .. } => {
                return Err(Error::InvalidSpec(
                    "scientific operation was not prepared".to_owned(),
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

    fn validate_and_lower(
        &mut self,
        base: &SceneSpec,
        scene: &molgfx_core::Scene,
        handles: &BTreeMap<RepresentationId, RepresentationHandle>,
        visuals: &BTreeMap<RepresentationId, crate::visual::ResolvedVisual>,
        properties: &BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
    ) -> Result<(), Error> {
        for selection in self.interactions.selections() {
            let _ = selection.fingerprint()?;
        }
        if let Change::Set(camera) = &self.camera {
            crate::spec::validate_camera(*camera)?;
        }
        let channels = self.interactions.channel_names(base);
        self.physical.reserve(self.representations.len());
        for (id, spec) in &self.representations {
            let handle = handles.get(id).copied().ok_or(Error::MissingId)?;
            let mut representation = scene
                .representation(handle)
                .cloned()
                .ok_or(Error::MissingId)?;
            if self.visual_replacements.contains(id) {
                let appearance = spec.prepare_appearance(crate::spec::lowering::Lowering {
                    properties,
                    channels: &channels,
                })?;
                let resolved = appearance.apply(&mut representation);
                let _ = self.visual_updates.insert(*id, resolved);
            } else {
                representation.material.opacity = spec.common.opacity;
                self.apply_parameters(*id, spec, visuals, &mut representation)?;
            }
            representation.visible = spec.common.visible;
            self.physical.push((handle, representation));
        }
        Ok(())
    }

    fn apply_parameters(
        &self,
        id: RepresentationId,
        spec: &RepresentationSpec,
        visuals: &BTreeMap<RepresentationId, crate::visual::ResolvedVisual>,
        representation: &mut Representation,
    ) -> Result<(), Error> {
        let Some(names) = self.parameter_updates.get(&id) else {
            return Ok(());
        };
        let source =
            spec.common.visual.as_ref().ok_or_else(|| {
                Error::InvalidSpec("visual parameters require a visual style".into())
            })?;
        let resolved = visuals
            .get(&id)
            .filter(|value| value.matches(source))
            .ok_or_else(|| {
                Error::InvalidSpec("resolved visual metadata does not match its source".into())
            })?;
        let style = representation.visual.as_mut().ok_or_else(|| {
            Error::InvalidSpec("resolved representation has no visual style".into())
        })?;
        for name in names {
            let binding = resolved.parameter(name).ok_or_else(|| {
                Error::InvalidSpec(format!("visual parameter '{name}' is not declared"))
            })?;
            let mut value = &binding.default;
            if let Some(replacement) = spec.common.parameters.get(name) {
                value = replacement;
            }
            apply_parameter(style, binding.slot, value)?;
        }
        Ok(())
    }

    pub(crate) fn commit(
        self,
        spec: &mut SceneSpec,
        scene: &mut molgfx_core::Scene,
        visuals: &mut BTreeMap<RepresentationId, crate::visual::ResolvedVisual>,
    ) -> Result<(), Error> {
        // Channel queries were already validated during preparation, so this
        // evaluates known-good queries against the scene that owns the columns.
        if self.interactions.touched() {
            let channels = self.interactions.channels(spec);
            let states = crate::scene::interaction::resolve_states(scene, &channels)?;
            crate::scene::interaction::install(scene, states)?;
        }
        scene.replace_representations(self.physical)?;
        for (id, representation) in self.representations {
            let _ = spec.representations.insert(id, representation);
        }
        for (id, visual) in self.visual_updates {
            if let Some(visual) = visual {
                let _ = visuals.insert(id, visual);
            } else {
                let _ = visuals.remove(&id);
            }
        }
        self.interactions.commit(spec);
        self.science.commit(spec);
        assign(&mut spec.camera, self.camera);
        advance_domain_revisions(spec, &self.operations);
        if !self.operations.is_empty() {
            spec.revision = spec.revision.wrapping_add(1);
        }
        Ok(())
    }
}

fn apply_parameter(
    style: &mut molgfx_core::VisualStyle,
    slot: usize,
    value: &crate::ParameterValue,
) -> Result<(), Error> {
    let invalid = || Error::InvalidSpec(format!("visual parameter slot {slot} has the wrong type"));
    match value {
        crate::ParameterValue::Scalar(value) => {
            let parameter = style.program().scalar_parameter(slot).ok_or_else(invalid)?;
            style
                .set_scalar(parameter, *value)
                .map_err(molgfx_core::CoreError::from)?;
        }
        crate::ParameterValue::Color(value) => {
            let parameter = style.program().color_parameter(slot).ok_or_else(invalid)?;
            style
                .set_color(parameter, value.to_linear_f32())
                .map_err(molgfx_core::CoreError::from)?;
        }
        crate::ParameterValue::Vector(value) => {
            let parameter = style.program().vector_parameter(slot).ok_or_else(invalid)?;
            style
                .set_vector(parameter, *value)
                .map_err(molgfx_core::CoreError::from)?;
        }
    }
    Ok(())
}

impl InteractionUpdates {
    fn set(&mut self, channel: &InteractionChannel, selection: Option<Selection>) {
        match channel {
            InteractionChannel::Selected => self.selected = Change::Set(selection),
            InteractionChannel::Hovered => self.hovered = Change::Set(selection),
            InteractionChannel::Focused => self.focus = Change::Set(selection),
            InteractionChannel::Muted => self.muted = Change::Set(selection),
            InteractionChannel::Hidden => self.hidden = Change::Set(selection),
            InteractionChannel::Custom(name) => {
                let _ = self.custom.insert(name.clone(), selection);
            }
        }
    }

    /// Channel names visible to a visual program after this patch commits:
    /// the base scene's channels, plus the ones this patch adds, minus the ones
    /// it clears. A style may read a channel the same patch declares.
    fn channel_names(&self, base: &SceneSpec) -> Vec<Box<str>> {
        let mut names: Vec<Box<str>> = base
            .custom_interactions
            .keys()
            .filter(|name| !matches!(self.custom.get(*name), Some(None)))
            .cloned()
            .collect();
        for (name, selection) in &self.custom {
            if selection.is_some() && !names.contains(name) {
                names.push(name.clone());
            }
        }
        names.sort_unstable();
        names
    }

    /// True when this patch changes any interaction channel.
    fn touched(&self) -> bool {
        !matches!(self.focus, Change::Unchanged)
            || !matches!(self.selected, Change::Unchanged)
            || !matches!(self.hovered, Change::Unchanged)
            || !matches!(self.muted, Change::Unchanged)
            || !matches!(self.hidden, Change::Unchanged)
            || !self.custom.is_empty()
    }

    /// The channels this patch leaves behind, paired with their queries.
    fn channels(
        &self,
        base: &SceneSpec,
    ) -> Vec<(crate::property::registry::StateChannel, Selection)> {
        let mut candidate = base.clone();
        self.clone_into_spec(&mut candidate);
        crate::scene::interaction::channels(&candidate)
    }

    fn clone_into_spec(&self, spec: &mut SceneSpec) {
        assign(&mut spec.focus, self.focus.clone());
        assign(&mut spec.selected, self.selected.clone());
        assign(&mut spec.hovered, self.hovered.clone());
        assign(&mut spec.muted, self.muted.clone());
        assign(&mut spec.hidden, self.hidden.clone());
        for (name, selection) in &self.custom {
            if let Some(selection) = selection {
                let _ = spec
                    .custom_interactions
                    .insert(name.clone(), selection.clone());
            } else {
                let _ = spec.custom_interactions.remove(name);
            }
        }
    }

    fn selections(&self) -> impl Iterator<Item = &Selection> {
        [
            self.focus.value(),
            self.selected.value(),
            self.hovered.value(),
            self.muted.value(),
            self.hidden.value(),
        ]
        .into_iter()
        .flatten()
        .chain(self.custom.values().filter_map(Option::as_ref))
    }

    fn commit(self, spec: &mut SceneSpec) {
        assign(&mut spec.focus, self.focus);
        assign(&mut spec.selected, self.selected);
        assign(&mut spec.hovered, self.hovered);
        assign(&mut spec.muted, self.muted);
        assign(&mut spec.hidden, self.hidden);
        for (name, selection) in self.custom {
            if let Some(selection) = selection {
                let _ = spec.custom_interactions.insert(name, selection);
            } else {
                let _ = spec.custom_interactions.remove(&name);
            }
        }
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

fn assign<T>(target: &mut Option<T>, update: Change<T>) {
    if let Change::Set(update) = update {
        *target = update;
    }
}

fn is_structural(operation: &PatchOperation) -> bool {
    matches!(
        operation,
        PatchOperation::AddRepresentation { .. }
            | PatchOperation::RemoveRepresentation { .. }
            | PatchOperation::ReplaceRepresentation { .. }
    )
}
