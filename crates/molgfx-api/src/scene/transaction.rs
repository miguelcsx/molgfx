//! Transaction-local collection of semantic scene edits.
//!
//! A transaction stages edits against a private candidate copy of the
//! specification, so every edit — adding or removing a representation as much
//! as changing an opacity — is validated the moment it is staged, and a later
//! edit in the same transaction sees the effect of an earlier one: a
//! representation added first can be recoloured second.
//!
//! New identities are the next after the highest in the candidate. Nothing is
//! reserved in the scene, so a transaction that is abandoned or fails to
//! commit leaves no gap and no trace: the next transaction allocates the same
//! identities again. The staged operations become one patch, applied as one
//! revision.

use crate::error::{Error, PatchError};
use crate::{
    AppearanceRuleId, AppearanceRuleSpec, ColorSpec, InteractionChannel, Parameter, ParameterType,
    PatchOperation, RepresentationId, RepresentationSpec, ScenePatch, SceneSpec, Selection,
    VisualStyle,
};

/// Semantic edits validated and committed as one atomic scene revision.
#[derive(Debug)]
pub struct SceneTransaction {
    base_revision: u64,
    operations: Vec<PatchOperation>,
    candidate: SceneSpec,
    /// The first error an infallible setter met, reported when committing.
    deferred: Option<Error>,
}

impl SceneTransaction {
    pub(crate) fn begin(spec: &SceneSpec) -> Self {
        Self {
            base_revision: spec.revision,
            operations: Vec::new(),
            candidate: spec.clone(),
            deferred: None,
        }
    }

    /// The scene revision this transaction was begun against.
    #[must_use]
    pub const fn base_revision(&self) -> u64 {
        self.base_revision
    }

    /// The specification as it will be once every staged edit applies.
    ///
    /// Its revision is still the base revision: the revision advances once,
    /// when the transaction commits.
    #[must_use]
    pub const fn spec(&self) -> &SceneSpec {
        &self.candidate
    }

    /// The operations staged so far, in order.
    #[must_use]
    pub fn operations(&self) -> &[PatchOperation] {
        &self.operations
    }

    /// Stages one operation after validating it against the staged state.
    ///
    /// # Errors
    ///
    /// Returns the validation error; the transaction is unchanged.
    pub fn stage(&mut self, operation: PatchOperation) -> Result<(), Error> {
        let mut candidate = self.candidate.clone();
        crate::scene::runtime::apply_operation(&mut candidate, &operation)?;
        crate::scene::runtime::validate_touched_domains(
            &candidate,
            std::slice::from_ref(&operation),
        )?;
        self.candidate = candidate;
        self.operations.push(operation);
        Ok(())
    }

    /// Stages a new representation and returns the identity it will have.
    ///
    /// A representation that names no structure targets the scene's only
    /// structure.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid representation, or when the scene has
    /// several structures and the representation names none.
    pub fn add(
        &mut self,
        representation: impl Into<RepresentationSpec>,
    ) -> Result<RepresentationId, Error> {
        let mut representation = representation.into();
        if representation.common.structure.is_none() {
            if self.candidate.structures.len() != 1 {
                return Err(Error::InvalidSpec(
                    "multi-structure scenes require an explicit representation structure"
                        .to_owned(),
                ));
            }
            representation.common.structure = self.candidate.structures.keys().next().copied();
        }
        let id = RepresentationId(next_id(
            self.candidate
                .representations
                .keys()
                .next_back()
                .map(|id| id.0),
            "representation",
        )?);
        self.stage(PatchOperation::AddRepresentation { id, representation })?;
        Ok(id)
    }

    /// Stages removal of a representation.
    ///
    /// # Errors
    ///
    /// Returns an error when the representation does not exist.
    pub fn remove(&mut self, id: RepresentationId) -> Result<(), Error> {
        self.stage(PatchOperation::RemoveRepresentation { id })
    }

    /// Stages a base-colour change.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown representation or an invalid colour.
    pub fn set_color(&mut self, id: RepresentationId, color: ColorSpec) -> Result<(), Error> {
        self.stage(PatchOperation::SetColor { id, color })
    }

    /// Stages a change of the query a representation draws.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown representation or a malformed query.
    pub fn set_target(
        &mut self,
        id: RepresentationId,
        target: impl Into<Selection>,
    ) -> Result<(), Error> {
        self.stage(PatchOperation::SetRepresentationTarget {
            id,
            target: target.into(),
        })
    }

    /// Stages a selection-scoped colour rule and returns its identity, which
    /// is higher than every existing rule's, so it takes precedence.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid rule or an unknown structure.
    pub fn add_appearance_rule(
        &mut self,
        rule: AppearanceRuleSpec,
    ) -> Result<AppearanceRuleId, Error> {
        let id = AppearanceRuleId(next_id(
            self.candidate.appearance.keys().next_back().map(|id| id.0),
            "appearance rule",
        )?);
        self.stage(PatchOperation::AddAppearanceRule { id, rule })?;
        Ok(id)
    }

    /// Stages replacement of a colour rule, keeping its precedence.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown rule or an invalid replacement.
    pub fn replace_appearance_rule(
        &mut self,
        id: AppearanceRuleId,
        rule: AppearanceRuleSpec,
    ) -> Result<(), Error> {
        self.stage(PatchOperation::ReplaceAppearanceRule { id, rule })
    }

    /// Stages removal of a colour rule.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown rule.
    pub fn remove_appearance_rule(&mut self, id: AppearanceRuleId) -> Result<(), Error> {
        self.stage(PatchOperation::RemoveAppearanceRule { id })
    }

    /// Stages a visibility change.
    pub fn set_visible(&mut self, id: RepresentationId, visible: bool) {
        self.stage_deferred(PatchOperation::SetVisibility { id, visible });
    }

    /// Stages an opacity change.
    pub fn set_opacity(&mut self, id: RepresentationId, opacity: f32) {
        self.stage_deferred(PatchOperation::SetOpacity { id, opacity });
    }

    /// Stages an immutable visual-program replacement.
    pub fn set_visual(&mut self, id: RepresentationId, visual: Option<VisualStyle>) {
        self.stage_deferred(PatchOperation::SetVisual { id, visual });
    }

    /// Stages one typed visual parameter update.
    pub fn set_parameter<T: ParameterType>(
        &mut self,
        id: RepresentationId,
        parameter: &Parameter<T>,
        value: T,
    ) {
        self.stage_deferred(PatchOperation::SetParameter {
            id,
            name: parameter.name().into(),
            value: Some(value.into_parameter_value()),
        });
    }

    /// Stages a focus change.
    pub fn focus(&mut self, selection: impl Into<Selection>) {
        self.stage_deferred(PatchOperation::SetFocus {
            selection: Some(selection.into()),
        });
    }

    /// Stages one semantic interaction-channel edit.
    pub fn set_interaction(&mut self, channel: InteractionChannel, selection: Option<Selection>) {
        self.stage_deferred(PatchOperation::SetInteraction { channel, selection });
    }

    /// Stages renderer-independent view state.
    pub fn set_camera(&mut self, camera: Option<molgfx_math::Camera>) {
        self.stage_deferred(PatchOperation::SetCamera { camera });
    }

    /// The staged operations as one patch against the base revision.
    ///
    /// # Errors
    ///
    /// Returns the first error an infallible setter met while staging.
    pub fn into_patch(self) -> Result<ScenePatch, Error> {
        if let Some(error) = self.deferred {
            return Err(error);
        }
        Ok(ScenePatch {
            base_revision: self.base_revision,
            operations: self.operations,
        })
    }

    /// Stages through a setter that reports its failure at commit instead.
    fn stage_deferred(&mut self, operation: PatchOperation) {
        if self.deferred.is_some() {
            return;
        }
        if let Err(error) = self.stage(operation) {
            self.deferred = Some(error);
        }
    }
}

fn next_id(last: Option<u64>, kind: &str) -> Result<u64, Error> {
    crate::fallback(last, 0)
        .checked_add(1)
        .ok_or_else(|| PatchError::Invalid(format!("{kind} identity space is exhausted")).into())
}

#[cfg(test)]
#[path = "transaction_tests.rs"]
mod tests;
