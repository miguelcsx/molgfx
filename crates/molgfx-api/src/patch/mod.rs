//! Atomic revision-checked semantic scene edits.

pub(crate) mod appearance_ops;
mod inverse;
pub(crate) mod plan;
pub(crate) mod science_ops;
#[cfg(test)]
mod tests;

use crate::appearance::AppearanceRuleSpec;
use crate::color::ColorSpec;
use crate::id::{
    AnnotationId, AppearanceRuleId, MeasurementId, RepresentationId, ScientificInteractionId,
    StructureId, TrajectoryId, VolumeId,
};
use crate::representation::Selection;
use crate::representation::form::RepresentationSpec;
use crate::science::{
    AnnotationSpec, MeasurementSpec, ScientificInteractionSpec, TrajectorySpec, VolumeSpec,
};
use crate::spec::{InteractionChannel, SceneSpec, StructureSource};
use crate::{ParameterValue, VisualStyle};
use serde::{Deserialize, Serialize};

/// One typed semantic change.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PatchOperation {
    /// Inserts a molecular source under a new ID.
    ///
    /// The patch announces the structure; coordinates stay in the runtime
    /// binding, so applying this operation requires the caller to have bound
    /// the source for `id` before resolving the scene.
    AddStructure {
        /// Stable identity assigned by the authoring scene.
        id: StructureId,
        /// Portable source descriptor; coordinates remain a runtime binding.
        source: StructureSource,
    },
    /// Inserts a representation under a new ID.
    AddRepresentation {
        /// Stable identity assigned by the authoring scene.
        id: RepresentationId,
        /// Complete immutable representation value.
        representation: RepresentationSpec,
    },
    /// Inserts a density volume whose grid is supplied separately.
    AddVolume {
        /// Stable volume identity.
        id: VolumeId,
        /// Immutable volume metadata.
        volume: VolumeSpec,
    },
    /// Removes a density volume.
    RemoveVolume {
        /// Volume to remove.
        id: VolumeId,
    },
    /// Inserts a semantic annotation.
    AddAnnotation {
        /// Stable annotation identity.
        id: AnnotationId,
        /// Immutable annotation value.
        annotation: AnnotationSpec,
    },
    /// Removes an annotation.
    RemoveAnnotation {
        /// Annotation to remove.
        id: AnnotationId,
    },
    /// Inserts a geometric measurement.
    AddMeasurement {
        /// Stable measurement identity.
        id: MeasurementId,
        /// Immutable measurement value.
        measurement: MeasurementSpec,
    },
    /// Removes a measurement.
    RemoveMeasurement {
        /// Measurement to remove.
        id: MeasurementId,
    },
    /// Inserts a caller-supplied scientific interaction.
    AddScientificInteraction {
        /// Stable scientific interaction identity.
        id: ScientificInteractionId,
        /// Immutable interaction value.
        interaction: ScientificInteractionSpec,
    },
    /// Removes a scientific interaction.
    RemoveScientificInteraction {
        /// Scientific interaction to remove.
        id: ScientificInteractionId,
    },
    /// Binds a trajectory descriptor to one structure.
    AddTrajectory {
        /// Stable trajectory identity.
        id: TrajectoryId,
        /// Immutable trajectory metadata.
        trajectory: TrajectorySpec,
    },
    /// Removes a trajectory binding.
    RemoveTrajectory {
        /// Trajectory to remove.
        id: TrajectoryId,
    },
    /// Removes a representation.
    RemoveRepresentation {
        /// Representation to remove.
        id: RepresentationId,
    },
    /// Replaces appearance and target atomically.
    ReplaceRepresentation {
        /// Representation to replace.
        id: RepresentationId,
        /// Complete replacement value.
        representation: RepresentationSpec,
    },
    /// Changes the molecular query a representation draws.
    ///
    /// Membership changes, so the representation's geometry is rebuilt; its
    /// appearance, identity and every other representation are untouched.
    SetRepresentationTarget {
        /// Representation to retarget.
        id: RepresentationId,
        /// New `MolFrame` query over the representation's structure.
        target: Selection,
    },
    /// Changes only a representation's base colour.
    ///
    /// Selection-scoped appearance rules still take precedence over it for the
    /// atoms they cover.
    SetColor {
        /// Representation to recolour.
        id: RepresentationId,
        /// New base colour.
        color: ColorSpec,
    },
    /// Adds a selection-scoped colour rule under a new ID.
    AddAppearanceRule {
        /// Stable identity; a higher identity wins where rules overlap.
        id: AppearanceRuleId,
        /// Complete rule value.
        rule: AppearanceRuleSpec,
    },
    /// Replaces a colour rule in place, keeping its precedence.
    ReplaceAppearanceRule {
        /// Rule to replace.
        id: AppearanceRuleId,
        /// Complete replacement value.
        rule: AppearanceRuleSpec,
    },
    /// Removes a colour rule, restoring whatever it covered.
    RemoveAppearanceRule {
        /// Rule to remove.
        id: AppearanceRuleId,
    },
    /// Changes only visibility.
    SetVisibility {
        /// Representation to change.
        id: RepresentationId,
        /// New visibility state.
        visible: bool,
    },
    /// Changes only material opacity.
    SetOpacity {
        /// Representation to change.
        id: RepresentationId,
        /// New opacity.
        opacity: f32,
    },
    /// Replaces the immutable visual expression graph.
    SetVisual {
        /// Representation to change.
        id: RepresentationId,
        /// New style, or `None` to restore built-in appearance.
        visual: Option<VisualStyle>,
    },
    /// Updates one typed visual parameter without changing its program.
    SetParameter {
        /// Representation whose uniform block changes.
        id: RepresentationId,
        /// Stable parameter identity.
        name: Box<str>,
        /// Override value, or `None` to restore the declaration default.
        value: Option<ParameterValue>,
    },
    /// Changes the focus query.
    SetFocus {
        /// New focus query, or `None` to clear focus.
        selection: Option<Selection>,
    },
    /// Replaces one GPU-resident semantic interaction channel.
    SetInteraction {
        /// Channel to update.
        channel: InteractionChannel,
        /// Canonical query, or `None` to clear the channel.
        selection: Option<Selection>,
    },
    /// Replaces renderer-independent view state.
    SetCamera {
        /// New explicit camera, or `None` to restore automatic framing.
        camera: Option<molgfx_math::Camera>,
    },
}

/// Atomic incremental update created against one scene revision.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ScenePatch {
    /// Required base revision.
    pub base_revision: u64,
    /// Ordered semantic operations.
    pub operations: Vec<PatchOperation>,
}

impl ScenePatch {
    /// Patch with no operations.
    #[must_use]
    pub const fn empty(base_revision: u64) -> Self {
        Self {
            base_revision,
            operations: Vec::new(),
        }
    }

    /// Deterministic JSON encoding for transport to Python or WASM.
    ///
    /// # Errors
    ///
    /// Returns an error if the patch cannot be serialized.
    pub fn to_json(&self) -> Result<String, crate::Error> {
        serde_json::to_string(self).map_err(crate::Error::from)
    }

    /// Parses one versioned semantic patch.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed JSON.
    pub fn from_json(source: &str) -> Result<Self, crate::Error> {
        serde_json::from_str(source).map_err(crate::Error::from)
    }

    /// Creates an undo patch against the exact base scene.
    ///
    /// # Errors
    ///
    /// Returns a conflict for a different base revision or a missing semantic ID.
    pub fn inverse(&self, base: &SceneSpec) -> Result<Self, crate::Error> {
        if self.base_revision != base.revision {
            return Err(crate::PatchError::RevisionConflict {
                expected: self.base_revision,
                actual: base.revision,
            }
            .into());
        }
        let mut state = base.clone();
        let mut operation_groups = Vec::with_capacity(self.operations.len());
        for operation in &self.operations {
            operation_groups.push(inverse::inverse_operations(operation, &state)?);
            crate::scene::runtime::apply_operation(&mut state, operation)?;
        }
        let operations = operation_groups.into_iter().rev().flatten().collect();
        Ok(Self {
            base_revision: if self.operations.is_empty() {
                base.revision
            } else {
                base.revision.wrapping_add(1)
            },
            operations,
        })
    }
}
