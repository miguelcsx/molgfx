//! Dirty-state resolution for independent scientific property channels.

use molgfx_core::{
    AtomProperty, Representation, RowDomain, Scene, StructureHandle, VisualAttributeRef,
};

type Resolved<'a> = (Option<&'a AtomProperty>, Option<&'a AtomProperty>, [u64; 2]);

pub(super) type VisualResolved = ([Option<VisualAttributeRef>; 4], [u64; 4]);

pub(super) fn resolve<'a>(
    scene: &'a Scene,
    representation: &Representation,
    structure: StructureHandle,
) -> Resolved<'a> {
    let handles = [
        representation.color.property_handle(),
        representation
            .appearance
            .map(|appearance| appearance.property),
    ];
    let properties = handles
        .map(|handle| handle.and_then(|handle| scene.property_for_structure(handle, structure)));
    let revisions = handles.map(|handle| {
        handle
            .and_then(|handle| scene.property_content_revision(handle))
            .into_iter()
            .fold(0, |_, revision| revision)
    });
    (properties[0], properties[1], revisions)
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
