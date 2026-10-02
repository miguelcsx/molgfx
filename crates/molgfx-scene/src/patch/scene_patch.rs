//! An atomic, revision-checked batch of operations.

use super::{PatchOperation, inverse};
use crate::spec::SceneSpec;
use serde::{Deserialize, Serialize};

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
            crate::scene::apply::apply_operation(&mut state, operation)?;
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
