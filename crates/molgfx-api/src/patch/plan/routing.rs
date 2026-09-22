//! Patch-routing predicate: which operations re-resolve the whole scene.

use crate::PatchOperation;

/// A structural operation changes the core scene's item tables, so it is
/// applied by resolving the candidate specification afresh rather than by
/// mutating one live table.
pub(super) fn is_structural(operation: &PatchOperation) -> bool {
    matches!(
        operation,
        PatchOperation::AddStructure { .. }
            | PatchOperation::AddRepresentation { .. }
            | PatchOperation::RemoveRepresentation { .. }
            | PatchOperation::ReplaceRepresentation { .. }
            | PatchOperation::AddVolume { .. }
            | PatchOperation::RemoveVolume { .. }
            | PatchOperation::AddAnnotation { .. }
            | PatchOperation::RemoveAnnotation { .. }
            | PatchOperation::AddMeasurement { .. }
            | PatchOperation::RemoveMeasurement { .. }
            | PatchOperation::AddScientificInteraction { .. }
            | PatchOperation::RemoveScientificInteraction { .. }
            | PatchOperation::AddTrajectory { .. }
            | PatchOperation::RemoveTrajectory { .. }
    )
}
