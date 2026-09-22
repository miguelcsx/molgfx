use super::*;
use crate::scene_gpu::record_cache::RecordCache;
use crate::testing::MockDevice;
use molgfx_core::{AtomSelection, DatasetId, RepresentationKind, Scene};

/// The render fixture's placement, the selection it draws, and that selection's
/// key — all real values, so the visibility key is never hand-assembled.
fn fixture(scene: &Scene) -> (molgfx_core::StructureHandle, molgfx_core::SelectionHandle) {
    let Some((structure, placed)) = scene.structures().next() else {
        panic!("the fixture places a structure")
    };
    let _ = placed;
    let Some((representation, value)) = scene.representations().next() else {
        panic!("the fixture has a representation")
    };
    let _ = representation;
    let Some(selection) = value.selection() else {
        panic!("the representation selects atoms")
    };
    (structure, selection)
}

fn key(
    scene: &Scene,
    lod_mode: u32,
    visual_enabled: bool,
    bond_break_length: u32,
) -> VisibilityKey {
    let (structure, selection) = fixture(scene);
    let Some((_, placed)) = scene.structures().next() else {
        panic!("the fixture places a structure")
    };
    let Some(value) = scene.representation(representation_row(scene)) else {
        panic!("the representation resolves")
    };
    VisibilityKey {
        records: RecordCache::<MockDevice>::key(
            scene,
            placed,
            structure,
            value,
            selection,
            crate::scene_gpu::asset::GpuAssetIdentity {
                dataset: DatasetId::LEGACY,
                coordinate_generation: 0,
                fingerprint: 0,
            },
            [0; 2],
        ),
        lod_mode,
        visual_enabled,
        bond_break_length,
    }
}

fn representation_row(scene: &Scene) -> molgfx_core::RepresentationHandle {
    let Some((handle, _)) = scene.representations().next() else {
        panic!("the fixture has a representation")
    };
    handle
}

fn fixture_scene() -> Scene {
    let structure = crate::engine::tests::structure();
    let mut scene = Scene::from_structure(&structure)
        .unwrap_or_else(|error| panic!("fixture scene builds: {error}"));
    let selection = scene.add_selection(AtomSelection::All);
    if let Err(error) = scene.represent(selection, RepresentationKind::Spacefill) {
        panic!("spacefill applies: {error}");
    }
    scene
}

fn counts(atoms: u32, bonds: u32) -> crate::scene_gpu::buffers::CullCountInput {
    crate::scene_gpu::buffers::CullCountInput {
        atoms,
        bonds,
        lod_mode: 0,
        bond_break_length: 0.0,
        visual_enabled: false,
        atom_bvh_nodes: 0,
        atom_bvh_indices: 0,
        bond_bvh_nodes: 0,
        bond_bvh_indices: 0,
    }
}

fn needed(
    scene: &Scene,
    policies: &[(u32, bool, u32)],
) -> std::collections::BTreeMap<VisibilityKey, (u32, u32, crate::scene_gpu::buffers::CullCountInput)>
{
    let mut map = std::collections::BTreeMap::new();
    for (lod_mode, visual_enabled, bond_break_length) in policies {
        let _ = map.insert(
            key(scene, *lod_mode, *visual_enabled, *bond_break_length),
            (4, 2, counts(4, 2)),
        );
    }
    map
}

#[test]
fn one_key_allocates_one_visible_set_however_many_representations_need_it() {
    let device = MockDevice::default();
    let queue = device.queue();
    let scene = fixture_scene();
    let mut cache = VisibilityCache::<MockDevice>::new();
    let policy = needed(&scene, &[(0, false, 0)]);
    cache
        .sync(&device, &queue, &policy)
        .unwrap_or_else(|error| panic!("visible sets allocate: {error}"));
    let before = {
        let Ok(buffers) = device.log.buffers.lock() else {
            panic!("buffer log lock")
        };
        buffers.len()
    };

    cache
        .sync(&device, &queue, &policy)
        .unwrap_or_else(|error| panic!("visible sets reuse: {error}"));
    let Ok(buffers) = device.log.buffers.lock() else {
        panic!("buffer log lock")
    };
    assert_eq!(
        buffers
            .iter()
            .filter(|(_, label, _)| *label == "cull counts")
            .count(),
        1,
        "one visibility key owns exactly one cull-count buffer"
    );
    assert_eq!(buffers.len(), before, "a repeated key allocates nothing");
}

#[test]
fn a_different_cull_policy_is_a_different_key() {
    let device = MockDevice::default();
    let queue = device.queue();
    let scene = fixture_scene();
    let mut cache = VisibilityCache::<MockDevice>::new();
    let policy = needed(
        &scene,
        &[(0, false, 0), (1, false, 0), (0, true, 0), (0, false, 3)],
    );
    cache
        .sync(&device, &queue, &policy)
        .unwrap_or_else(|error| panic!("visible sets allocate: {error}"));
    let Ok(buffers) = device.log.buffers.lock() else {
        panic!("buffer log lock")
    };
    assert_eq!(
        buffers
            .iter()
            .filter(|(_, label, _)| *label == "cull counts")
            .count(),
        4,
        "each distinct cull policy needs its own visible set"
    );
}

#[test]
fn an_unneeded_key_releases_its_visible_set() {
    let device = MockDevice::default();
    let queue = device.queue();
    let scene = fixture_scene();
    let mut cache = VisibilityCache::<MockDevice>::new();
    let policy = needed(&scene, &[(0, false, 0)]);
    let only = *policy.keys().next().unwrap_or(&key(&scene, 0, false, 0));
    cache
        .sync(&device, &queue, &policy)
        .unwrap_or_else(|error| panic!("visible sets allocate: {error}"));
    assert!(cache.get(only).is_some());
    cache
        .sync(&device, &queue, &std::collections::BTreeMap::new())
        .unwrap_or_else(|error| panic!("visible sets release: {error}"));
    assert!(
        cache.get(only).is_none(),
        "a key no representation needs is released"
    );
    assert_eq!(cache.keys().count(), 0);
}
