//! Atomic revision-checked semantic scene edits.

pub(crate) mod plan;
pub(crate) mod science_ops;

use crate::id::{
    AnnotationId, MeasurementId, RepresentationId, ScientificInteractionId, StructureId,
    TrajectoryId, VolumeId,
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
            operation_groups.push(inverse_operations(operation, &state)?);
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

type Inverse = Result<Vec<PatchOperation>, crate::Error>;

fn inverse_operations(operation: &PatchOperation, base: &SceneSpec) -> Inverse {
    if let Some(operations) = inverse_science(operation, base)? {
        return Ok(operations);
    }
    if let Some(operations) = inverse_representation(operation, base)? {
        return Ok(operations);
    }
    let one = |operation| vec![operation];
    let structure = |id: StructureId, base: &SceneSpec| {
        let source = base
            .structures
            .get(&id)
            .cloned()
            .ok_or(crate::PatchError::MissingId);
        source.map(|source| vec![PatchOperation::AddStructure { id, source }])
    };
    Ok(match operation {
        PatchOperation::AddStructure { id, .. } => structure(*id, base)?,
        PatchOperation::SetFocus { .. } => one(PatchOperation::SetFocus {
            selection: base.focus.clone(),
        }),
        PatchOperation::SetInteraction { channel, .. } => one(PatchOperation::SetInteraction {
            channel: channel.clone(),
            selection: interaction(base, channel).cloned(),
        }),
        PatchOperation::SetCamera { .. } => one(PatchOperation::SetCamera {
            camera: base.camera,
        }),
        _ => {
            return Err(crate::Error::InvalidSpec(
                "patch inversion was not dispatched".to_owned(),
            ));
        }
    })
}

fn inverse_representation(
    operation: &PatchOperation,
    base: &SceneSpec,
) -> Result<Option<Vec<PatchOperation>>, crate::Error> {
    let one = |operation| Some(vec![operation]);
    Ok(match operation {
        PatchOperation::AddRepresentation { id, .. } => {
            one(PatchOperation::RemoveRepresentation { id: *id })
        }
        PatchOperation::RemoveRepresentation { id } => one(PatchOperation::AddRepresentation {
            id: *id,
            representation: base
                .representations
                .get(id)
                .cloned()
                .ok_or(crate::PatchError::MissingId)?,
        }),
        PatchOperation::ReplaceRepresentation { id, .. } => {
            one(PatchOperation::ReplaceRepresentation {
                id: *id,
                representation: base
                    .representations
                    .get(id)
                    .cloned()
                    .ok_or(crate::PatchError::MissingId)?,
            })
        }
        PatchOperation::SetVisibility { id, .. } => one(PatchOperation::SetVisibility {
            id: *id,
            visible: base
                .representations
                .get(id)
                .ok_or(crate::PatchError::MissingId)?
                .common
                .visible,
        }),
        PatchOperation::SetOpacity { id, .. } => one(PatchOperation::SetOpacity {
            id: *id,
            opacity: base
                .representations
                .get(id)
                .ok_or(crate::PatchError::MissingId)?
                .opacity(),
        }),
        PatchOperation::SetVisual { id, .. } => {
            let representation = base
                .representations
                .get(id)
                .ok_or(crate::PatchError::MissingId)?;
            let mut operations = Vec::with_capacity(representation.common.parameters.len() + 1);
            operations.push(PatchOperation::SetVisual {
                id: *id,
                visual: representation.common.visual.clone(),
            });
            operations.extend(
                representation
                    .common
                    .parameters
                    .iter()
                    .map(|(name, value)| PatchOperation::SetParameter {
                        id: *id,
                        name: name.clone(),
                        value: Some(value.clone()),
                    }),
            );
            Some(operations)
        }
        PatchOperation::SetParameter { id, name, .. } => one(PatchOperation::SetParameter {
            id: *id,
            name: name.clone(),
            value: base
                .representations
                .get(id)
                .ok_or(crate::PatchError::MissingId)?
                .common
                .parameters
                .get(name)
                .cloned(),
        }),
        _ => None,
    })
}

fn inverse_science(
    operation: &PatchOperation,
    base: &SceneSpec,
) -> Result<Option<Vec<PatchOperation>>, crate::Error> {
    let one = |operation| Some(vec![operation]);
    Ok(match operation {
        PatchOperation::AddVolume { id, .. } => one(PatchOperation::RemoveVolume { id: *id }),
        PatchOperation::RemoveVolume { id } => one(PatchOperation::AddVolume {
            id: *id,
            volume: base
                .volumes
                .get(id)
                .cloned()
                .ok_or(crate::PatchError::MissingId)?,
        }),
        PatchOperation::AddAnnotation { id, .. } => {
            one(PatchOperation::RemoveAnnotation { id: *id })
        }
        PatchOperation::RemoveAnnotation { id } => one(PatchOperation::AddAnnotation {
            id: *id,
            annotation: base
                .annotations
                .get(id)
                .cloned()
                .ok_or(crate::PatchError::MissingId)?,
        }),
        PatchOperation::AddMeasurement { id, .. } => {
            one(PatchOperation::RemoveMeasurement { id: *id })
        }
        PatchOperation::RemoveMeasurement { id } => one(PatchOperation::AddMeasurement {
            id: *id,
            measurement: base
                .measurements
                .get(id)
                .cloned()
                .ok_or(crate::PatchError::MissingId)?,
        }),
        PatchOperation::AddScientificInteraction { id, .. } => {
            one(PatchOperation::RemoveScientificInteraction { id: *id })
        }
        PatchOperation::RemoveScientificInteraction { id } => {
            one(PatchOperation::AddScientificInteraction {
                id: *id,
                interaction: base
                    .scientific_interactions
                    .get(id)
                    .cloned()
                    .ok_or(crate::PatchError::MissingId)?,
            })
        }
        PatchOperation::AddTrajectory { id, .. } => {
            one(PatchOperation::RemoveTrajectory { id: *id })
        }
        PatchOperation::RemoveTrajectory { id } => one(PatchOperation::AddTrajectory {
            id: *id,
            trajectory: base
                .trajectories
                .get(id)
                .cloned()
                .ok_or(crate::PatchError::MissingId)?,
        }),
        _ => None,
    })
}

fn interaction<'a>(scene: &'a SceneSpec, channel: &InteractionChannel) -> Option<&'a Selection> {
    match channel {
        InteractionChannel::Selected => scene.selected.as_ref(),
        InteractionChannel::Hovered => scene.hovered.as_ref(),
        InteractionChannel::Focused => scene.focus.as_ref(),
        InteractionChannel::Muted => scene.muted.as_ref(),
        InteractionChannel::Hidden => scene.hidden.as_ref(),
        InteractionChannel::Custom(name) => scene.custom_interactions.get(name),
    }
}
