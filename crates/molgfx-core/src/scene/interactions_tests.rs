use crate::{
    EntityKind, EntityRef, InteractionAnchor, InteractionEdge, InteractionGeometry,
    InteractionKind, Scene, fixture,
};
use molgfx_math::Vec3;

fn interaction(scene: &Scene) -> InteractionEdge {
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture owner exists")
    };
    let start = InteractionAnchor::world(Vec3::ZERO).unwrap_or_else(|error| panic!("{error}"));
    let end = InteractionAnchor::world(Vec3::new(9.0, 2.0, 0.0))
        .unwrap_or_else(|error| panic!("{error}"));
    let geometry = InteractionGeometry::new(9.22, None).unwrap_or_else(|error| panic!("{error}"));
    InteractionEdge::new(
        owner,
        start,
        end,
        InteractionKind::SaltBridge,
        geometry,
        "test",
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn interaction_table_uses_stable_generational_handles_and_revisions() {
    let mut scene =
        Scene::from_structure(&fixture::structure()).unwrap_or_else(|error| panic!("{error}"));
    let initial = scene.interaction_revision();
    let edge = interaction(&scene);
    let handle = scene
        .add_interaction(edge)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(scene.interaction_revision() > initial);
    assert_eq!(scene.interaction_count(), 1);
    let Some((resolved, edge)) = scene.interaction_for_entity(EntityRef {
        structure: edge_owner(&scene),
        kind: EntityKind::Edge,
        index: Scene::interaction_row(handle),
    }) else {
        panic!("picked edge resolves")
    };
    assert_eq!(resolved, handle);
    assert_eq!(edge.provenance(), "test");
    let after_add = scene.interaction_revision();
    scene
        .interaction_mut(handle)
        .unwrap_or_else(|| panic!("interaction resolves"))
        .set_visible(false);
    assert!(scene.interaction_revision() > after_add);
    assert!(scene.remove_interaction(handle).is_some());
    assert!(scene.interaction(handle).is_none());
}

fn edge_owner(scene: &Scene) -> crate::StructureHandle {
    scene
        .structures()
        .next()
        .map_or_else(|| panic!("fixture owner exists"), |(handle, _)| handle)
}

#[test]
fn interaction_endpoints_extend_the_scene_world_bound() {
    let mut scene =
        Scene::from_structure(&fixture::structure()).unwrap_or_else(|error| panic!("{error}"));
    let edge = interaction(&scene);
    scene
        .add_interaction(edge)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(scene.world_aabb().max.x >= 9.0);
}
