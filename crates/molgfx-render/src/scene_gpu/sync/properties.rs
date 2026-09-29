//! Dirty-state resolution for independent physical property channels.

use molgfx_core::{Representation, RowDomain, Scene, StructureHandle, VisualAttributeRef};
use molgfx_geometry::ColorContext;

/// The colour context and the revisions that invalidate geometry baked from it.
///
/// The first revision folds every column the colour scheme and its overlay
/// read; the second is the appearance mapping's own column.
type Resolved<'a> = (ColorContext<'a>, [u64; 2]);

pub(super) type VisualResolved = ([Option<VisualAttributeRef>; 4], [u64; 4]);

pub(super) fn resolve<'a>(
    scene: &'a Scene,
    representation: &Representation,
    structure: StructureHandle,
) -> Resolved<'a> {
    let columns = representation.color_columns();
    let revision = |handle: molgfx_core::AtomPropertyHandle| content_revision(scene, handle);
    let scheme_revision = columns
        .handles()
        .filter(|handle| Some(*handle) != columns.appearance)
        .fold(0_u64, |folded, handle| mix(folded, revision(handle)));
    let appearance_revision = match columns.appearance {
        Some(handle) => revision(handle),
        None => 0,
    };
    (
        ColorContext::new(scene, structure),
        [scheme_revision, appearance_revision],
    )
}

/// A column's content revision, zero when the column is gone.
fn content_revision(scene: &Scene, handle: molgfx_core::AtomPropertyHandle) -> u64 {
    let Some(revision) = scene.property_content_revision(handle) else {
        return 0;
    };
    revision
}

/// Folds one revision into a running digest, order-sensitively.
///
/// A 64-bit multiplicative mix: two different revision sets collide with
/// probability `2^-64`, and the same set always folds to the same value.
const fn mix(folded: u64, revision: u64) -> u64 {
    (folded ^ revision.wrapping_add(0x9e37_79b9_7f4a_7c15)).wrapping_mul(0x0100_0000_01b3)
}

pub(super) fn resolve_visual(
    scene: &Scene,
    representation: &Representation,
    structure: StructureHandle,
) -> VisualResolved {
    let handles = representation.visual.as_ref().map_or_else(
        || [None; 4],
        |style| {
            let mut handles = [None; 4];
            for (slot, handle) in style.program().attributes().iter().enumerate() {
                if slot == handles.len() {
                    break;
                }
                handles[slot] = Some(*handle);
            }
            handles
        },
    );
    let handles = handles.map(|handle| {
        handle.filter(|reference| match *reference {
            VisualAttributeRef::Attribute { handle, kind } => scene
                .attribute_for_domain(handle, RowDomain::Atoms(structure))
                .is_some_and(|attribute| attribute.kind() == kind),
            VisualAttributeRef::LegacyScalar(handle) => {
                scene.property_for_structure(handle, structure).is_some()
            }
            VisualAttributeRef::Column { .. } => false,
        })
    });
    let revisions = handles.map(|reference| match reference {
        Some(VisualAttributeRef::Attribute { handle, .. }) => {
            scene.attribute_change(handle).map_or(0, |change| change.0)
        }
        Some(VisualAttributeRef::LegacyScalar(handle)) => scene
            .property_content_revision(handle)
            .into_iter()
            .fold(0, |_, revision| revision),
        Some(VisualAttributeRef::Column { .. }) | None => 0,
    });
    (handles, revisions)
}
