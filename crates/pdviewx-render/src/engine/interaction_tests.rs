use super::tests::{camera, engine, structure};
use pdviewx_core::{
    InteractionAnchor, InteractionDirection, InteractionEdge, InteractionGeometry, InteractionKind,
    Scene,
};
use pdviewx_math::Vec3;

fn interaction_scene() -> Scene {
    let source = structure();
    let mut scene = Scene::from_structure(&source).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture structure exists")
    };
    for (index, kind) in [
        InteractionKind::HydrogenBond,
        InteractionKind::SaltBridge,
        InteractionKind::PiStacking,
        InteractionKind::Hydrophobic,
        InteractionKind::MetalCoordination,
    ]
    .into_iter()
    .enumerate()
    {
        let y = u16::try_from(index).map_or(f32::from(u16::MAX), f32::from) - 2.0;
        let start = InteractionAnchor::world(Vec3::new(-2.0, y, 0.0))
            .unwrap_or_else(|error| panic!("{error}"));
        let end = InteractionAnchor::world(Vec3::new(2.0, y, 0.0))
            .unwrap_or_else(|error| panic!("{error}"));
        let geometry =
            InteractionGeometry::new(4.0, None).unwrap_or_else(|error| panic!("{error}"));
        let edge = InteractionEdge::new(owner, start, end, kind, geometry, "test:pdbiox")
            .unwrap_or_else(|error| panic!("{error}"))
            .with_direction(InteractionDirection::Forward);
        scene
            .add_interaction(edge)
            .unwrap_or_else(|error| panic!("{error}"));
    }
    scene
}

#[test]
fn all_interactions_share_one_indirect_draw() {
    let scene = interaction_scene();
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let draws = engine
        .device
        .log
        .indirect_draws
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(draws.len(), 1);
}

#[test]
fn unchanged_interactions_add_no_steady_frame_uploads() {
    let scene = interaction_scene();
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let before = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(writes.len() - before, 1, "only frame uniforms change");
}
