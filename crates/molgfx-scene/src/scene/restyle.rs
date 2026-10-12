//! Atomic form changes that preserve authored appearance and layer identity.

use super::Scene;
use crate::rep::RepresentationApplicability;
use crate::{Error, PatchOperation, RepresentationId, RepresentationSpec, ScenePatch};

impl Scene {
    /// Checks an exact form and selection against the scene's molecular data.
    ///
    /// # Errors
    /// Returns an error for an ambiguous or missing structure, invalid query,
    /// invalid controls, or geometry extraction failure.
    pub fn representation_applicability(
        &self,
        representation: impl Into<RepresentationSpec>,
    ) -> Result<RepresentationApplicability, Error> {
        let representation = representation.into();
        let structure = if let Some(structure) = representation.structure_id() {
            Some(structure)
        } else if self.structures.len() == 1 {
            self.structures.first_key_value().map(|(id, _)| *id)
        } else {
            None
        }
        .ok_or_else(|| {
            Error::InvalidSpec("representation requires an unambiguous bound structure".to_owned())
        })?;
        self.representation_review(structure)?
            .applicability(&representation)
    }

    /// Captures shared molecular data and the current pose for a worker review.
    /// This retains handles without scanning or copying coordinate columns.
    ///
    /// # Errors
    /// Rejects an unbound molecular structure.
    pub fn representation_review(
        &self,
        structure: crate::StructureId,
    ) -> Result<crate::rep::RepresentationReview, Error> {
        let placed = self
            .structures
            .keys()
            .zip(self.resolved.structures())
            .find_map(|(id, (_, placed))| (*id == structure).then_some(placed))
            .ok_or(Error::MissingId)?;
        Ok(crate::rep::RepresentationReview::new(placed))
    }

    /// Prepares a replacement form over the layer's existing target.
    ///
    /// Colour, opacity, visibility, visual program, parameters and structure
    /// remain those of the layer. Only geometric controls come from `form`.
    ///
    /// # Errors
    /// Rejects unknown layers and forms that cannot draw their target.
    pub fn prepare_representation_form(
        &self,
        id: RepresentationId,
        form: impl Into<RepresentationSpec>,
    ) -> Result<RepresentationSpec, Error> {
        let mut replacement = self
            .spec
            .representations
            .get(&id)
            .cloned()
            .ok_or_else(|| Error::InvalidSpec("representation does not exist".to_owned()))?;
        replacement.form = form.into().form;
        let applicability = self.representation_applicability(replacement.clone())?;
        if let Some(requirement) = applicability.missing {
            return Err(Error::InvalidSpec(format!(
                "{} cannot draw this target: requires {requirement:?}",
                replacement.form_name()
            )));
        }
        Ok(replacement)
    }

    /// Changes a layer's geometric form as one atomic, reviewable patch.
    ///
    /// # Errors
    /// Rejects an unsuitable target or invalid visual program without removing
    /// or changing the previous representation.
    pub fn set_representation_form(
        &mut self,
        id: RepresentationId,
        form: impl Into<RepresentationSpec>,
    ) -> Result<(), Error> {
        let representation = self.prepare_representation_form(id, form)?;
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::ReplaceRepresentation { id, representation }],
        })
    }
}
