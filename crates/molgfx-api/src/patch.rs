//! Atomic revision-checked semantic scene edits.

use crate::id::RepresentationId;
use crate::representation::Selection;
use crate::spec::{InteractionChannel, RepresentationSpec, SceneSpec};
use crate::{ParameterValue, VisualStyle};
use serde::{Deserialize, Serialize};

/// One typed semantic change.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PatchOperation {
    /// Inserts a representation under a new ID.
    AddRepresentation {
        /// Stable identity assigned by the authoring scene.
        id: RepresentationId,
        /// Complete immutable representation value.
        representation: RepresentationSpec,
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
            crate::scene_runtime::apply_operation(&mut state, operation)?;
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

fn inverse_operations(
    operation: &PatchOperation,
    base: &SceneSpec,
) -> Result<Vec<PatchOperation>, crate::Error> {
    let one = |operation| vec![operation];
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
                .visible,
        }),
        PatchOperation::SetOpacity { id, .. } => one(PatchOperation::SetOpacity {
            id: *id,
            opacity: base
                .representations
                .get(id)
                .ok_or(crate::PatchError::MissingId)?
                .opacity,
        }),
        PatchOperation::SetVisual { id, .. } => {
            let representation = base
                .representations
                .get(id)
                .ok_or(crate::PatchError::MissingId)?;
            let mut operations = Vec::with_capacity(representation.parameters.len() + 1);
            operations.push(PatchOperation::SetVisual {
                id: *id,
                visual: representation.visual.clone(),
            });
            operations.extend(representation.parameters.iter().map(|(name, value)| {
                PatchOperation::SetParameter {
                    id: *id,
                    name: name.clone(),
                    value: Some(value.clone()),
                }
            }));
            operations
        }
        PatchOperation::SetParameter { id, name, .. } => one(PatchOperation::SetParameter {
            id: *id,
            name: name.clone(),
            value: base
                .representations
                .get(id)
                .ok_or(crate::PatchError::MissingId)?
                .parameters
                .get(name)
                .cloned(),
        }),
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
