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

#[test]
fn static_contact_inputs_cover_both_placements_and_survive_cold_reopening_without_losing_their_scope()
 {
    let structure = fixture::structure();
    let mut scene = Scene::from_structure(&structure).expect("scene");
    let first = edge_owner(&scene);
    let second = scene
        .add_source(scene.structure(first).expect("first").source.clone())
        .expect("second");
    let edge = interaction(&scene)
        .with_static_coordinates([Some(first), Some(second)], &scene)
        .expect("scope");
    assert!(edge.coordinates_match(&scene));
    let edge = scene.add_interaction(edge).expect("edge");
    scene.structure_mut(second).expect("second").model_to_world =
        molgfx_math::Mat4::from_translation(Vec3::X);
    assert!(
        !scene
            .interaction(edge)
            .expect("edge")
            .coordinates_match(&scene)
    );
    let description = scene.describe();
    let encoded = serde_json::to_string(&description).expect("encode");
    let description = serde_json::from_str(&encoded).expect("decode");
    let mut reopened = Scene::from_description(
        &description,
        crate::SceneDescriptionSources {
            structures: &[structure.clone(), structure.clone()],
            volumes: &[],
            segmentations: &[],
            atom_properties: &[],
            meshes: &[],
        },
    )
    .expect("reopen");
    let (edge, value) = reopened.interactions().next().expect("edge");
    assert!(!value.coordinates_match(&reopened));
    reopened
        .structure_mut(second)
        .expect("second")
        .model_to_world = molgfx_math::Mat4::IDENTITY;
    assert!(
        reopened
            .interaction(edge)
            .expect("edge")
            .coordinates_match(&reopened)
    );
    let original = Scene::from_structure(&fixture::structure()).expect("independent source");
    let independent = interaction(&original)
        .with_static_coordinates([Some(first), None], &original)
        .expect("scope");
    let revision = scene.interaction_revision();
    assert!(scene.add_interaction(independent).is_err());
    assert_eq!(scene.interaction_revision(), revision);
}
