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
            | PatchOperation::AddSegmentation { .. }
            | PatchOperation::RemoveSegmentation { .. }
            | PatchOperation::RemoveVolume { .. }
            | PatchOperation::AddAnnotation { .. }
            | PatchOperation::RemoveAnnotation { .. }
            | PatchOperation::AddMeasurement { .. }
            | PatchOperation::RemoveMeasurement { .. }
            | PatchOperation::AddInteraction { .. }
            | PatchOperation::RemoveInteraction { .. }
            | PatchOperation::AddTrajectory { .. }
            | PatchOperation::RemoveTrajectory { .. }
 | PatchOperation::AddEllipsoids { .. }
 | PatchOperation::RemoveEllipsoids { .. }
 | PatchOperation::AddPlane { .. }
 | PatchOperation::RemovePlane { .. }
 // Domain values are retained in the scene extension namespace.
            // They still participate in full scene resolution so history and
            // renderer projections observe one atomic semantic transition.
            | PatchOperation::SetAssembly { .. }
            | PatchOperation::SetFitting { .. }
            | PatchOperation::SetValidation { .. }
            | PatchOperation::RestoreSnapshot(_)
    )
}

/// Whether the operation edits an SNFG representation.
///
/// Symbols are primitives re-emitted whenever the scene resolves, not a
/// physical representation a local patch could rewrite in place, so any edit
/// to one resolves afresh.
pub(super) fn edits_snfg(spec: &crate::SceneSpec, operation: &PatchOperation) -> bool {
    let (PatchOperation::SetVisibility { id, .. }
    | PatchOperation::SetOpacity { id, .. }
    | PatchOperation::SetColor { id, .. }
    | PatchOperation::SetRepresentationTarget { id, .. }
    | PatchOperation::SetVisual { id, .. }
    | PatchOperation::SetParameter { id, .. }) = operation
    else {
        return false;
    };
    spec.representations.get(id).is_some_and(|representation| {
        matches!(
            representation.form,
            crate::representation::form::RepresentationFormSpec::Snfg { .. }
        )
    })
}
