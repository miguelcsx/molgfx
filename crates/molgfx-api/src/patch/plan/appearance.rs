//! Appearance-rule edits staged by one patch.
//!
//! Rules are recorded against a copy of the rule table, and every structure a
//! rule enters or leaves is marked. Only marked structures are re-evaluated,
//! and only their representations receive a new overlay, so an edit to one
//! structure's colours costs nothing for the others.

use super::PatchInputs;
use crate::AppearanceRuleSpec;
use crate::error::{Error, PatchError};
use crate::id::{AppearanceRuleId, StructureId};
use crate::scene::appearance::PreparedClasses;
use crate::spec::{PatchOperation, SceneSpec};
use molgfx_core::StructureHandle;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct AppearanceUpdates {
    rules: Option<BTreeMap<AppearanceRuleId, AppearanceRuleSpec>>,
    touched: BTreeSet<StructureId>,
    pub(super) prepared: Option<Vec<PreparedClasses>>,
}

impl AppearanceUpdates {
    /// Stages one appearance operation, or returns `false` for any other.
    pub(super) fn apply(
        &mut self,
        spec: &SceneSpec,
        operation: &PatchOperation,
    ) -> Result<bool, Error> {
        match operation {
            PatchOperation::AddAppearanceRule { id, rule } => {
                crate::patch::appearance_ops::validate(spec, rule)?;
                let rules = self.rules.get_or_insert_with(|| spec.appearance.clone());
                if rules.insert(*id, rule.clone()).is_some() {
                    return Err(PatchError::Invalid(
                        "appearance rule ID already exists".to_owned(),
                    )
                    .into());
                }
                let _ = self.touched.insert(rule.structure);
            }
            PatchOperation::ReplaceAppearanceRule { id, rule } => {
                crate::patch::appearance_ops::validate(spec, rule)?;
                let rules = self.rules.get_or_insert_with(|| spec.appearance.clone());
                let Some(previous) = rules.insert(*id, rule.clone()) else {
                    return Err(PatchError::MissingId.into());
                };
                let _ = self.touched.insert(previous.structure);
                let _ = self.touched.insert(rule.structure);
            }
            PatchOperation::RemoveAppearanceRule { id } => {
                let rules = self.rules.get_or_insert_with(|| spec.appearance.clone());
                let Some(previous) = rules.remove(id) else {
                    return Err(PatchError::MissingId.into());
                };
                let _ = self.touched.insert(previous.structure);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Evaluates every touched structure's final rules.
    pub(super) fn prepare(
        &mut self,
        inputs: PatchInputs<'_>,
        handles: &BTreeMap<StructureId, StructureHandle>,
    ) -> Result<(), Error> {
        let Some(rules) = &self.rules else {
            return Ok(());
        };
        self.prepared = Some(crate::scene::appearance::prepare(
            rules,
            &self.touched,
            inputs.structures,
            handles,
            inputs.rows,
        )?);
        Ok(())
    }

    /// Writes the staged rule table into `spec`.
    pub(super) fn commit_spec(
        rules: Option<BTreeMap<AppearanceRuleId, AppearanceRuleSpec>>,
        spec: &mut SceneSpec,
    ) {
        if let Some(rules) = rules {
            spec.appearance = rules;
        }
    }

    /// Splits the staged table from the prepared columns for commit.
    pub(super) fn into_parts(
        self,
    ) -> (
        Option<BTreeMap<AppearanceRuleId, AppearanceRuleSpec>>,
        Option<Vec<PreparedClasses>>,
    ) {
        (self.rules, self.prepared)
    }
}
