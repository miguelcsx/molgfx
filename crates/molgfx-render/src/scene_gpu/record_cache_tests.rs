use super::*;
use molgfx_core::{
    AtomSelection, Representation, RepresentationKind, RepresentationTarget, Scene,
    SelectionHandle, StructureHandle,
};

#[test]
fn trajectory_endpoints_change_the_quality_key_but_playback_time_does_not() {
    use molgfx_core::{TrajectoryFrame, TrajectorySegment};
    use std::sync::Arc;

    let source = crate::engine::tests::structure();
    let mut scene = Scene::from_structure(&source).expect("fixture scene");
    let handle = scene.structures().next().expect("structure").0;
    let selection = scene.add_selection(AtomSelection::All);
    let representation = Representation::new(
        RepresentationTarget::Selection(selection),
        RepresentationKind::Spacefill,
    );
    let key = |scene: &Scene| {
        let placed = scene.structures().next().expect("structure").1;
        RecordCache::<crate::testing::MockDevice>::key(
            scene,
            placed,
            handle,
            &representation,
            selection,
            GpuAssetIdentity::from_placed(placed),
            [0; 2],
        )
    };
    let static_key = key(&scene);
    let start =
        TrajectoryFrame::new(0, 0.0, Arc::from([[0.0; 3]; 3]), "start").expect("start frame");
    let end = TrajectoryFrame::new(1, 1.0, Arc::from([[3.0; 3]; 3]), "end").expect("end frame");
    scene
        .set_trajectory_segment(
            handle,
            TrajectorySegment::new(start, end, 0.25).expect("segment"),
        )
        .expect("bind trajectory");
    let moving_key = key(&scene);
    assert_ne!(static_key, moving_key);
    scene
        .set_trajectory_time(handle, 0.75)
        .expect("advance time");
    assert_eq!(moving_key, key(&scene));
    scene.clear_trajectory(handle).expect("clear trajectory");
    assert_eq!(static_key, key(&scene));
}

#[test]
fn a_set_larger_than_the_budget_stays_resident_instead_of_repacking_every_frame() {
    use crate::testing::MockDevice;
    use crate::{DerivedCache, DerivedCacheBudget};

    let source = crate::engine::tests::structure();
    let mut scene = Scene::from_structure(&source).expect("fixture scene");
    let handle = scene.structures().next().expect("structure").0;
    let selection = scene.add_selection(AtomSelection::All);
    let representation = Representation::new(
        RepresentationTarget::Selection(selection),
        RepresentationKind::Spacefill,
    );
    let placed = scene.structures().next().expect("structure").1;
    let key = RecordCache::<MockDevice>::key(
        &scene,
        placed,
        handle,
        &representation,
        selection,
        GpuAssetIdentity::from_placed(placed),
        [0; 2],
    );
    let mut ledger = DerivedCache::new(DerivedCacheBudget {
        cpu_bytes: 1024,
        gpu_bytes: 1024,
    });
    let set = RecordSet::<MockDevice> {
        atoms_capacity: 1 << 20,
        ..RecordSet::default()
    };
    let mut cache = RecordCache::<MockDevice>::new();
    cache.admit(&mut ledger, key, set, 1);
    assert!(cache.get(key).is_some(), "refused sets must stay resident");
    assert_eq!(ledger.usage().gpu_bytes, 0, "and stay unaccounted");
}

fn selection_key(scene: &Scene, handle: SelectionHandle, topology: u64) -> SelectionKey {
    SelectionKey {
        structure: structure_handle(),
        identity: match scene.selection_fingerprint(handle) {
            Some(fingerprint) => SelectionIdentity::Query(fingerprint),
            None => SelectionIdentity::Mask {
                scene: scene.cache_identity(),
                handle,
            },
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
fn equal_mask_handles_in_different_scenes_never_share_records() {
    let structure = crate::engine::tests::structure();
    let mut first = Scene::from_structure(&structure).expect("first scene");
    let mut second = Scene::from_structure(&structure).expect("second scene");
    let carbon = first.add_selection(AtomSelection::Sparse(vec![0]));
    let oxygen = second.add_selection(AtomSelection::Sparse(vec![1]));
    assert_eq!(carbon, oxygen, "each scene starts its own handle arena");
    assert_ne!(
        selection_key(&first, carbon, 0),
        selection_key(&second, oxygen, 0)
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
    colored.color = molgfx_core::ColorScheme::Uniform(molgfx_math::Rgba8::opaque(1, 2, 3));
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
    let mut sampled = representation.clone();
    sampled.params.point_samples = 1024;
    assert_eq!(
        base,
        RecordState::new(&sampled),
        "sample density changes drawing, without repacking atoms"
    );
}
