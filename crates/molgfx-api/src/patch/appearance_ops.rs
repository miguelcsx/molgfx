//! Ordered patch application for selection-scoped appearance rules.

use crate::scene::runtime::{insert_unique, remove_existing};
use crate::{Error, PatchError, PatchOperation, SceneSpec};

/// Applies one appearance operation to a candidate specification.
///
/// Returns `false` for any other operation, which the caller dispatches on.
///
/// # Errors
///
/// Returns an error for an invalid rule, a rule naming an unknown structure,
/// a duplicate identity, or an absent one.
pub(crate) fn apply(candidate: &mut SceneSpec, operation: &PatchOperation) -> Result<bool, Error> {
    match operation {
        PatchOperation::AddAppearanceRule { id, rule } => {
            validate(candidate, rule)?;
            insert_unique(
                &mut candidate.appearance,
                *id,
                rule.clone(),
                "appearance rule",
            )?;
        }
        PatchOperation::ReplaceAppearanceRule { id, rule } => {
            validate(candidate, rule)?;
            let Some(existing) = candidate.appearance.get_mut(id) else {
                return Err(PatchError::MissingId.into());
            };
            *existing = rule.clone();
        }
        PatchOperation::RemoveAppearanceRule { id } => {
            remove_existing(&mut candidate.appearance, id)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub(crate) fn validate(spec: &SceneSpec, rule: &crate::AppearanceRuleSpec) -> Result<(), Error> {
    rule.validate()?;
    if !spec.structures.contains_key(&rule.structure) {
        return Err(Error::InvalidSpec(
            "appearance rule targets an unknown structure".to_owned(),
        ));
    }
    Ok(())
}
