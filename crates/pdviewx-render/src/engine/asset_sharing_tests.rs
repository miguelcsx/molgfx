use super::tests::{camera, engine, structure};
use crate::RenderError;
use pdviewx_core::{AtomSelection, DatasetId, RepresentationKind, Scene, StructureAsset};

#[test]
fn distinct_assets_share_one_physical_buffer_without_overlapping_ranges() {
    let source = structure();
    let first = match StructureAsset::new(DatasetId::new(101), &source) {
        Ok(asset) => asset,
        Err(error) => panic!("first fixture asset builds: {error}"),
    };
    let second = match StructureAsset::new(DatasetId::new(102), &source) {
        Ok(asset) => asset,
        Err(error) => panic!("second fixture asset builds: {error}"),
    };
    let mut scene = Scene::from_asset(&first);
    scene.add_asset(&second);
    represent_all(&mut scene);
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("distinct asset scene renders: {error}");
    }
    let layouts = engine.scene_gpu.asset_layouts(|buffer| buffer.id);
    assert_eq!(engine.scene_gpu.asset_counts().0, 2);
    assert_eq!(layouts.len(), 2);
    assert_eq!(layouts[0].buffers, layouts[1].buffers);
    assert_ne!(layouts[0].offsets, layouts[1].offsets);
    let first_end = layouts[0].offsets[2] + layouts[0].lengths[2];
    assert!(first_end <= layouts[1].offsets[0]);
    assert_eq!(
        engine.residency_metrics().immutable_assets.physical_buffers,
        1
    );
}

#[test]
fn placements_share_asset_buffers_and_release_after_the_last_owner() {
    let source = structure();
    let asset = match StructureAsset::new(DatasetId::new(41), &source) {
        Ok(asset) => asset,
        Err(error) => panic!("fixture asset builds: {error}"),
    };
    let mut scene = Scene::from_asset(&asset);
    let Some((first, _)) = scene.structures().next() else {
        panic!("first placement exists")
    };
    let second = scene.add_asset(&asset);
    represent_all(&mut scene);
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("shared asset scene renders: {error}");
    }

    let (asset_count, placement_count, resident_bytes) = engine.scene_gpu.asset_counts();
    assert_eq!((asset_count, placement_count), (1, 2));
    assert!(resident_bytes > 0);
    let metrics = engine.residency_metrics();
    assert_eq!(metrics.machine.resident_resources, 2);
    assert!(metrics.machine.resident_payload_bytes >= resident_bytes);
    assert_eq!(metrics.immutable_assets.physical_buffers, 1);
    assert_eq!(metrics.immutable_assets.resident_bytes, resident_bytes);
    assert_eq!(metrics.immutable_assets.writes, 3);
    let layouts = engine.scene_gpu.asset_layouts(|buffer| buffer.id);
    assert_eq!(layouts.len(), 2);
    assert_eq!(layouts[0].buffers, layouts[1].buffers);
    assert_eq!(layouts[0].offsets, layouts[1].offsets);
    assert_eq!(layouts[0].lengths, layouts[1].lengths);
    assert_ne!(layouts[0].model, layouts[1].model);
    assert_non_overlapping(layouts[0].offsets, layouts[0].lengths);
    assert_ranges_are_consumed(&engine, layouts[0].buffers[0], layouts[0].offsets);
    assert_single_writes(
        &engine,
        layouts[0].buffers[0],
        [layouts[0].model, layouts[1].model],
    );
    let steady_metrics = engine.residency_metrics().immutable_assets;
    let steady_buffer_count = immutable_buffer_count(&engine);
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("steady shared scene renders: {error}");
    }
    let after_steady = engine.residency_metrics().immutable_assets;
    assert_eq!(after_steady.writes, steady_metrics.writes);
    assert_eq!(after_steady.relocations, steady_metrics.relocations);
    assert_eq!(immutable_buffer_count(&engine), steady_buffer_count);

    assert!(scene.remove_structure(first).is_some());
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("remaining placement renders: {error}");
    }
    assert_eq!(engine.scene_gpu.asset_counts().0, 1);
    let remaining = engine.scene_gpu.asset_layouts(|buffer| buffer.id);
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].buffers, layouts[0].buffers);
    assert_eq!(remaining[0].offsets, layouts[0].offsets);

    assert!(scene.remove_structure(second).is_some());
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("empty scene renders: {error}");
    }
    assert_eq!(engine.scene_gpu.asset_counts(), (0, 0, 0));
    let released = engine.residency_metrics().immutable_assets;
    assert_eq!(released.resident_bytes, 0);
    assert_eq!(released.physical_buffers, 1);
}

fn assert_single_writes(
    engine: &super::Engine<crate::testing::MockDevice>,
    arena: u32,
    models: [u32; 2],
) {
    let writes = match engine.device.log.writes.lock() {
        Ok(writes) => writes,
        Err(error) => panic!("write log locks: {error}"),
    };
    assert_eq!(
        writes.iter().filter(|write| write.0 == arena).count(),
        3,
        "coordinates and both BVH columns upload once into one arena"
    );
    for model in models {
        assert_eq!(writes.iter().filter(|write| write.0 == model).count(), 1);
    }
}

fn represent_all(scene: &mut Scene) {
    let selection = scene.add_selection(AtomSelection::All);
    if let Err(error) = scene.represent(selection, RepresentationKind::BallAndStick) {
        panic!("fixture representation applies: {error}");
    }
}

fn assert_non_overlapping(offsets: [u64; 3], lengths: [u64; 3]) {
    assert!(offsets[0] + lengths[0] <= offsets[1]);
    assert!(offsets[1] + lengths[1] <= offsets[2]);
}

fn immutable_buffer_count(engine: &super::Engine<crate::testing::MockDevice>) -> usize {
    match engine.device.log.buffers.lock() {
        Ok(buffers) => buffers
            .iter()
            .filter(|buffer| buffer.1 == "immutable asset arena")
            .count(),
        Err(error) => panic!("buffer log locks: {error}"),
    }
}

fn assert_ranges_are_consumed(
    engine: &super::Engine<crate::testing::MockDevice>,
    arena: u32,
    offsets: [u64; 3],
) {
    let bindings = match engine.device.log.buffer_bindings.lock() {
        Ok(bindings) => bindings,
        Err(error) => panic!("binding log locks: {error}"),
    };
    for offset in offsets {
        assert!(
            bindings
                .iter()
                .any(|binding| binding.2 == arena && binding.3 == offset && binding.4 > 0),
            "draw or culling binding must consume every immutable arena range"
        );
    }
}

#[test]
fn conflicting_content_for_one_dataset_is_a_typed_error() {
    let source = structure();
    let first_source = edited_structure(&source, 1.0);
    let second_source = edited_structure(&source, 5.0);
    let first = match StructureAsset::new(DatasetId::new(77), &first_source) {
        Ok(asset) => asset,
        Err(error) => panic!("first asset builds: {error}"),
    };
    let second = match StructureAsset::new(DatasetId::new(77), &second_source) {
        Ok(asset) => asset,
        Err(error) => panic!("second asset builds: {error}"),
    };
    let mut scene = Scene::from_asset(&first);
    scene.add_asset(&second);
    let mut engine = engine();
    let result = engine.render(&scene, &camera());
    assert!(matches!(
        result,
        Err(RenderError::AssetIdentityCollision { dataset, .. })
            if dataset == DatasetId::new(77)
    ));
}

fn edited_structure(source: &pdbiox::Structure, offset: f32) -> pdbiox::Structure {
    let mut editor = match source.edit_coordinates(&pdbiox::ExecutionContext::default()) {
        Ok(editor) => editor,
        Err(error) => panic!("coordinate edit is admitted: {error}"),
    };
    let Some(coordinates) = editor.positions_mut(pdbiox::ModelIndex::new(0)) else {
        panic!("fixture coordinates are mutable")
    };
    let Some(position) = coordinates.first_mut() else {
        panic!("fixture has coordinates")
    };
    position[0] += offset;
    match editor.commit() {
        Ok(structure) => structure,
        Err(findings) => panic!("coordinate edit commits: {findings:?}"),
    }
}
