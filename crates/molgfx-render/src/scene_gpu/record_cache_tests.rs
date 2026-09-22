use super::*;
use molgfx_core::{
    AtomSelection, Representation, RepresentationKind, RepresentationTarget, Scene,
    SelectionHandle, StructureHandle,
};

fn selection_key(scene: &Scene, handle: SelectionHandle, topology: u64) -> SelectionKey {
    SelectionKey {
        structure: structure_handle(),
        identity: match scene.selection_fingerprint(handle) {
            Some(fingerprint) => SelectionIdentity::Query(fingerprint),
            None => SelectionIdentity::Mask(handle),
        },
        topology,
    }
}

/// A real structure handle from the shared render fixture.
fn structure_handle() -> StructureHandle {
    let scene = Scene::from_structure(&crate::engine::tests::structure())
        .unwrap_or_else(|error| panic!("fixture scene builds: {error}"));
    let Some((handle, _)) = scene.structures().next() else {
        panic!("the fixture places a structure")
    };
    handle
}

#[test]
fn equal_queries_over_one_topology_share_a_selection_key() {
    let structure = crate::engine::tests::structure();
    let mut scene = Scene::from_structure(&structure)
        .unwrap_or_else(|error| panic!("fixture scene builds: {error}"));
    let first = scene.select_str("element C").expect("first query");
    let second = scene.select_str("element C").expect("second query");
    let other = scene.select_str("element N").expect("third query");
    assert_eq!(
        selection_key(&scene, first, 5),
        selection_key(&scene, second, 5),
        "equal queries must share the identity derived resources key on"
    );
    assert_ne!(
        selection_key(&scene, first, 5),
        selection_key(&scene, other, 5),
        "different queries must not alias"
    );
    assert_ne!(
        selection_key(&scene, first, 5),
        selection_key(&scene, first, 6),
        "a rewritten bond topology must not reuse the previous records"
    );
}

#[test]
fn distinct_masks_without_a_query_never_alias() {
    let structure = crate::engine::tests::structure();
    let mut scene = Scene::from_structure(&structure)
        .unwrap_or_else(|error| panic!("fixture scene builds: {error}"));
    let first = scene.add_selection(AtomSelection::Sparse(vec![0, 2]));
    let second = scene.add_selection(AtomSelection::Sparse(vec![0, 2]));
    assert_eq!(
        scene.selection_fingerprint(first),
        None,
        "a hand-built mask is not any single query"
    );
    assert_ne!(
        selection_key(&scene, first, 5),
        selection_key(&scene, second, 5),
        "two hand-built masks must stay distinct even when equal"
    );
}

#[test]
fn record_state_distinguishes_every_input_that_forces_a_repack() {
    let structure = crate::engine::tests::structure();
    let mut scene = Scene::from_structure(&structure)
        .unwrap_or_else(|error| panic!("fixture scene builds: {error}"));
    let selection = scene.add_selection(AtomSelection::All);
    let representation = Representation::new(
        RepresentationTarget::Selection(selection),
        RepresentationKind::Spacefill,
    );
    let base = RecordState::new(&representation);
    let mut ball = representation.clone();
    ball.kind = RepresentationKind::BallAndStick;
    let mut scaled = representation.clone();
    scaled.params.radius_scale = 0.5;
    let mut colored = representation.clone();
    colored.color = molgfx_core::ColorScheme::ByChain;
    let mut opaque = representation.clone();
    opaque.material.opacity = 0.25;
    assert_eq!(base, RecordState::new(&representation));
    assert_ne!(base, RecordState::new(&ball), "kind forces a repack");
    assert_ne!(
        base,
        RecordState::new(&scaled),
        "radius scale forces a repack"
    );
    assert!(base < RecordState::new(&ball) || base > RecordState::new(&ball));
    // Colour and opacity are applied on the GPU, so neither may invalidate a
    // packed record: repacking them would defeat every shared-record scheme.
    assert_eq!(
        base,
        RecordState::new(&colored),
        "a colour scheme is not a repack"
    );
    assert_eq!(base, RecordState::new(&opaque), "opacity is not a repack");
}
