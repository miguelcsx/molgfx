//! Transaction-local collection of semantic scene edits.

use crate::{
    InteractionChannel, Parameter, ParameterType, PatchOperation, RepresentationId, ScenePatch,
    Selection, VisualStyle,
};

/// Semantic edits validated and committed as one atomic scene revision.
#[derive(Debug)]
pub struct SceneTransaction {
    pub(crate) patch: ScenePatch,
}

impl SceneTransaction {
    /// Stages a visibility change.
    pub fn set_visible(&mut self, id: RepresentationId, visible: bool) {
        self.patch
            .operations
            .push(PatchOperation::SetVisibility { id, visible });
    }

    /// Stages an opacity change.
    pub fn set_opacity(&mut self, id: RepresentationId, opacity: f32) {
        self.patch
            .operations
            .push(PatchOperation::SetOpacity { id, opacity });
    }

    /// Stages an immutable visual-program replacement.
    pub fn set_visual(&mut self, id: RepresentationId, visual: Option<VisualStyle>) {
        self.patch
            .operations
            .push(PatchOperation::SetVisual { id, visual });
    }

    /// Stages one typed visual parameter update.
    pub fn set_parameter<T: ParameterType>(
        &mut self,
        id: RepresentationId,
        parameter: &Parameter<T>,
        value: T,
    ) {
        self.patch.operations.push(PatchOperation::SetParameter {
            id,
            name: parameter.name().into(),
            value: Some(value.into_parameter_value()),
        });
    }

    /// Stages a focus change.
    pub fn focus(&mut self, selection: impl Into<Selection>) {
        self.patch.operations.push(PatchOperation::SetFocus {
            selection: Some(selection.into()),
        });
    }

    /// Stages one semantic interaction-channel edit.
    pub fn set_interaction(&mut self, channel: InteractionChannel, selection: Option<Selection>) {
        self.patch
            .operations
            .push(PatchOperation::SetInteraction { channel, selection });
    }

    /// Stages renderer-independent view state.
    pub fn set_camera(&mut self, camera: Option<molgfx_math::Camera>) {
        self.patch
            .operations
            .push(PatchOperation::SetCamera { camera });
    }
}
