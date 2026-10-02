//! Direct edits of a scene that each advance its revision.

use super::Scene;
use crate::error::Error;
use crate::id::RepresentationId;
use crate::representation::Selection;
use crate::spec::{InteractionChannel, PatchOperation, ScenePatch};

impl Scene {
    /// Shows or hides one representation through an atomic one-operation patch.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown ID or failed runtime update.
    pub fn set_visible(&mut self, id: RepresentationId, visible: bool) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetVisibility { id, visible }],
        })
    }

    /// Sets representation opacity without rebuilding molecular records.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown ID or opacity outside zero to one.
    pub fn set_opacity(&mut self, id: RepresentationId, opacity: f32) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetOpacity { id, opacity }],
        })
    }

    /// Replaces a representation's immutable visual program without rebuilding geometry.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown ID or invalid expression graph.
    pub fn set_visual(
        &mut self,
        id: RepresentationId,
        visual: Option<crate::VisualStyle>,
    ) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetVisual { id, visual }],
        })
    }

    /// Updates one typed visual parameter without recompiling its program.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown representation, parameter, or invalid value.
    pub fn set_parameter<T: crate::ParameterType>(
        &mut self,
        id: RepresentationId,
        parameter: &crate::Parameter<T>,
        value: T,
    ) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetParameter {
                id,
                name: parameter.name().into(),
                value: Some(value.into_parameter_value()),
            }],
        })
    }

    /// Focuses a molecular subject and de-emphasizes its distant context.
    ///
    /// The selected atoms receive the focused interaction bit, and — unless the
    /// muted channel has been set explicitly — whole residues beyond the
    /// surrounding shell receive the muted bit. Both are ordinary state bits, so
    /// a visual style reads them through `visual.state(..)` and the camera frames
    /// them through [`Self::focus_bounds`]. No hidden representation is inserted.
    ///
    /// # Errors
    ///
    /// Returns an error if the query or runtime update is invalid.
    pub fn focus(&mut self, selection: impl Into<Selection>) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetFocus {
                selection: Some(selection.into()),
            }],
        })
    }

    /// Replaces one semantic interaction channel without rebuilding geometry.
    ///
    /// # Errors
    ///
    /// Returns an error if the update would make the scene invalid.
    pub fn set_interaction(
        &mut self,
        channel: InteractionChannel,
        selection: Option<Selection>,
    ) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetInteraction { channel, selection }],
        })
    }

    /// Sets an explicit semantic camera or restores automatic framing.
    ///
    /// # Errors
    ///
    /// Returns an error if the camera is non-finite or degenerate.
    pub fn set_camera(&mut self, camera: Option<molgfx_math::Camera>) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetCamera { camera }],
        })
    }
}
