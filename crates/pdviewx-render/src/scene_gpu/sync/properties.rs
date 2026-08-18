//! Dirty-state resolution for independent scientific property channels.

use pdviewx_core::{AtomProperty, Representation, Scene, StructureHandle};

type Resolved<'a> = (Option<&'a AtomProperty>, Option<&'a AtomProperty>, [u64; 2]);

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
    let revisions = handles.map(|handle| match handle {
        Some(handle) => match scene.property_content_revision(handle) {
            Some(revision) => revision,
            None => 0,
        },
        None => 0,
    });
    (properties[0], properties[1], revisions)
}
