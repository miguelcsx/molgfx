use super::tests::{camera, engine};
use crate::testing::MockDevice;
use molgfx_core::{
    AtomSelection, ClipPlane, OccupancyStream, Representation, ScalarVolume, Scene, SegmentStyle,
    SegmentStyleTable, SegmentationStyle, SegmentedVolume, TrajectoryFrame, TrajectorySegment,
    VolumeRendering, VolumeSlice, VolumeStyle,
};
use molgfx_math::{Mat4, Rgba8, Vec3};
use std::sync::Arc;

#[test]
fn temporal_occupancy_stays_gpu_resident_and_runs_only_for_new_samples() {
    let source = super::tests::structure();
    let mut scene = Scene::new();
    let structure = scene
        .add_structure(&source)
        .unwrap_or_else(|error| panic!("fixture structure adds: {error}"));
    let start = TrajectoryFrame::new(
        0,
        0.0,
        Arc::from([[-2.0, 0.0, 0.0], [0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
        "occupancy:start",
    )
    .unwrap_or_else(|error| panic!("start frame validates: {error}"));
    let end = TrajectoryFrame::new(
        1,
        1.0,
        Arc::from([[-1.0, 1.0, 0.0], [0.0, 1.5, 0.0], [1.0, 1.0, 0.0]]),
        "occupancy:end",
    )
    .unwrap_or_else(|error| panic!("end frame validates: {error}"));
    scene
        .set_trajectory_segment(
            structure,
            TrajectorySegment::new(start, end, 0.25)
                .unwrap_or_else(|error| panic!("segment validates: {error}")),
        )
        .unwrap_or_else(|error| panic!("trajectory binds: {error}"));
    let stream = OccupancyStream::new(
        [16; 3],
        Vec3::splat(-4.0),
        Vec3::splat(0.5),
        0.98,
        1.0,
        32.0,
    )
    .unwrap_or_else(|error| panic!("occupancy validates: {error}"));
    let volume = scene
        .add_occupancy_stream(structure, &AtomSelection::All, stream)
        .unwrap_or_else(|error| panic!("occupancy binds: {error}"));
    scene
        .represent(volume, Representation::volume())
        .unwrap_or_else(|error| panic!("occupancy represents: {error}"));

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("initial occupancy renders: {error}"));
    let after_initial = dispatch_count(&engine);
    assert!(after_initial >= 5, "trajectory and occupancy compute run");
    let texture_uploads = engine
        .device
        .log
        .texture_writes
        .lock()
        .unwrap_or_else(|error| panic!("texture log lock: {error}"));
    assert!(
        texture_uploads.is_empty(),
        "occupancy has no host voxel upload"
    );
    drop(texture_uploads);

    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("steady occupancy renders: {error}"));
    let after_steady = dispatch_count(&engine);
    scene
        .set_trajectory_time(structure, 0.75)
        .unwrap_or_else(|error| panic!("trajectory advances: {error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("advanced occupancy renders: {error}"));
    let after_advance = dispatch_count(&engine);
    assert_eq!(
        after_advance - after_steady,
        after_steady - after_initial + 5,
        "one interpolation and four occupancy kernels accompany normal frame work"
    );
    let replacement =
        OccupancyStream::new([8; 3], Vec3::splat(-2.0), Vec3::splat(0.5), 0.95, 1.0, 16.0)
            .unwrap_or_else(|error| panic!("replacement occupancy validates: {error}"));
    scene
        .replace_occupancy_stream(volume, structure, &AtomSelection::Range(0..1), replacement)
        .unwrap_or_else(|error| panic!("occupancy replaces: {error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("replacement occupancy renders: {error}"));
    let after_replace = dispatch_count(&engine);
    assert_eq!(
        after_replace - after_advance,
        after_steady - after_initial + 5,
        "replacement clears/resolves a fresh grid and refreshes normal visibility state"
    );
}

fn dispatch_count(engine: &super::Engine<MockDevice>) -> usize {
    engine.device.log.dispatches.lock().map_or_else(
        |error| panic!("dispatch log lock: {error}"),
        |log| log.len(),
    )
}

#[test]
fn density_volume_uploads_the_callers_values_once_and_draws_through_oit() {
    let values: Arc<[f32]> = (0..64)
        .map(|index| f32::from(u8::try_from(index).map_or(u8::MAX, |value| value)) / 63.0)
        .collect();
    let source_pointer = values.as_ptr() as usize;
    let volume = match ScalarVolume::from_spacing([4, 4, 4], Vec3::splat(-1.5), Vec3::ONE, values) {
        Ok(volume) => volume,
        Err(error) => panic!("volume builds: {error}"),
    };
    let mut scene = Scene::new();
    let volume = scene.add_volume(volume);
    if let Err(error) = scene.represent(volume, Representation::volume()) {
        panic!("volume representation applies: {error}")
    }
    if let Err(error) = scene.represent(
        volume,
        Representation::volume().volume_style(VolumeStyle::isosurface()),
    ) {
        panic!("isosurface representation applies: {error}")
    }
    let plane = match ClipPlane::from_point_normal(Vec3::ZERO, Vec3::Z) {
        Ok(plane) => plane,
        Err(error) => panic!("slice plane builds: {error}"),
    };
    if let Err(error) = scene.represent(
        volume,
        Representation::volume().volume_style(VolumeStyle::slice(VolumeSlice::new(plane))),
    ) {
        panic!("slice representation applies: {error}")
    }
    if let Err(error) = scene.represent(
        volume,
        Representation::volume().volume_style(VolumeStyle::medium()),
    ) {
        panic!("medium representation applies: {error}")
    }
    if let Err(error) = scene.represent(
        volume,
        Representation::volume().volume_style(VolumeStyle::liquid_surface()),
    ) {
        panic!("liquid representation applies: {error}")
    }
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("volume renders: {error}")
    }
    let uploads = match engine.device.log.texture_writes.lock() {
        Ok(uploads) => uploads.clone(),
        Err(error) => panic!("log lock: {error}"),
    };
    assert_eq!(uploads.len(), 2);
    assert_eq!(uploads[0], ("caller density volume", 256, source_pointer));
    assert_eq!(uploads[1].0, "density empty-space bounds");
    assert_eq!(
        engine
            .scene_gpu
            .volume_draws()
            .map(|(rendering, _)| rendering)
            .collect::<Vec<_>>(),
        [
            VolumeRendering::Direct,
            VolumeRendering::Isosurface,
            VolumeRendering::Slice,
            VolumeRendering::Medium,
            VolumeRendering::LiquidSurface,
        ]
    );
    assert!(engine.scene_gpu.has_translucency());
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("unchanged volume renders: {error}")
    }
    let Ok(uploads) = engine.device.log.texture_writes.lock() else {
        panic!("log lock")
    };
    assert_eq!(uploads.len(), 2, "representations share one resident grid");
}

#[test]
fn independent_scalar_channels_keep_distinct_residency_and_styles() {
    let first_values: Arc<[f32]> = Arc::from([0.1; 8]);
    let second_values: Arc<[f32]> = Arc::from([0.9; 8]);
    let pointers = [
        first_values.as_ptr() as usize,
        second_values.as_ptr() as usize,
    ];
    let first = match ScalarVolume::new([2, 2, 2], Mat4::IDENTITY, first_values) {
        Ok(volume) => volume,
        Err(error) => panic!("first channel builds: {error}"),
    };
    let second = match ScalarVolume::new([2, 2, 2], Mat4::IDENTITY, second_values) {
        Ok(volume) => volume,
        Err(error) => panic!("second channel builds: {error}"),
    };
    let mut scene = Scene::new();
    let first = scene.add_volume(first);
    let second = scene.add_volume(second);
    let first_representation = match scene.represent(first, Representation::volume()) {
        Ok(handle) => handle,
        Err(error) => panic!("first channel represents: {error}"),
    };
    let second_representation = match scene.represent(
        second,
        Representation::volume().volume_style(VolumeStyle::isosurface()),
    ) {
        Ok(handle) => handle,
        Err(error) => panic!("second channel represents: {error}"),
    };
    if let Some(style) = scene.representation_mut(first_representation) {
        style.volume.opacity_scale = 0.4;
    }
    if let Some(style) = scene.representation_mut(second_representation) {
        style.volume.opacity_scale = 3.0;
    }
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("channels render: {error}")
    }
    let uploads = match engine.device.log.texture_writes.lock() {
        Ok(uploads) => uploads.clone(),
        Err(error) => panic!("upload log lock: {error}"),
    };
    assert_eq!(uploads.len(), 4);
    assert_eq!(
        uploads
            .iter()
            .filter(|upload| upload.0 == "caller density volume")
            .map(|upload| upload.2)
            .collect::<Vec<_>>(),
        pointers
    );
    assert_eq!(engine.scene_gpu.volume_draws().count(), 2);
}

#[test]
fn categorical_labels_upload_once_share_residency_and_keep_styles_independent() {
    let labels: std::sync::Arc<[u32]> = Arc::from([0, 1, 1, 2, 2, 0, 2, 1]);
    let pointer = labels.as_ptr() as usize;
    let volume = match SegmentedVolume::new([2, 2, 2], Mat4::IDENTITY, labels) {
        Ok(volume) => volume,
        Err(error) => panic!("categorical volume builds: {error}"),
    };
    let mut scene = Scene::new();
    let volume = scene.add_segmented_volume(volume);
    let first_styles =
        match SegmentStyleTable::new(&[SegmentStyle::new(1, Rgba8::opaque(220, 30, 30), 0.8)]) {
            Ok(styles) => styles,
            Err(error) => panic!("first styles build: {error}"),
        };
    let second_styles =
        match SegmentStyleTable::new(&[SegmentStyle::new(100, Rgba8::opaque(30, 80, 220), 0.6)]) {
            Ok(styles) => styles,
            Err(error) => panic!("second styles build: {error}"),
        };
    let first = match scene.represent(volume, segmentation(first_styles)) {
        Ok(handle) => handle,
        Err(error) => panic!("first segmentation represents: {error}"),
    };
    let second = match scene.represent(volume, segmentation(second_styles)) {
        Ok(handle) => handle,
        Err(error) => panic!("second segmentation represents: {error}"),
    };
    if let Some(representation) = scene.representation_mut(second) {
        representation.segmentation.opacity_scale = 0.4;
        let plane = match ClipPlane::from_point_normal(Vec3::ZERO, Vec3::Z) {
            Ok(plane) => plane,
            Err(error) => panic!("categorical slice plane builds: {error}"),
        };
        representation.segmentation.slice = Some(VolumeSlice::new(plane));
    }
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("categorical segmentation renders: {error}")
    }
    let uploads = match engine.device.log.texture_writes.lock() {
        Ok(uploads) => uploads.clone(),
        Err(error) => panic!("upload log lock: {error}"),
    };
    assert_eq!(
        uploads,
        vec![("caller categorical segmentation", 32, pointer)]
    );
    assert_eq!(
        engine
            .scene_gpu
            .segmentation_draws()
            .map(|(key, _)| key)
            .collect::<Vec<_>>(),
        [
            crate::scene_gpu::SegmentationPipelineKey::Direct,
            crate::scene_gpu::SegmentationPipelineKey::HashSlice,
        ]
    );
    assert!(engine.scene_gpu.has_translucency());

    if let Some(representation) = scene.representation_mut(first) {
        representation.segmentation.opacity_scale = 0.25;
    }
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("style-only categorical edit renders: {error}")
    }
    let Ok(uploads) = engine.device.log.texture_writes.lock() else {
        panic!("upload log lock")
    };
    assert_eq!(uploads.len(), 1, "style edits do not reupload labels");
}

#[test]
fn categorical_picking_returns_the_typed_volume_segment_identity() {
    let mut scene = Scene::new();
    let volume = match SegmentedVolume::new([2, 2, 2], Mat4::IDENTITY, Arc::from([1; 8])) {
        Ok(volume) => volume,
        Err(error) => panic!("categorical volume builds: {error}"),
    };
    let volume = scene.add_segmented_volume(volume);
    let styles = match SegmentStyleTable::new(&[SegmentStyle::new(1, Rgba8::WHITE, 1.0)]) {
        Ok(styles) => styles,
        Err(error) => panic!("styles build: {error}"),
    };
    if let Err(error) = scene.represent(volume, segmentation(styles)) {
        panic!("segmentation represents: {error}")
    }
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("categorical segmentation renders: {error}")
    }
    if let Ok(mut source) = engine.device.log.segment_pick_source.lock() {
        *source = 0;
    }
    if let Ok(mut label) = engine.device.log.segment_pick_label.lock() {
        *label = 1;
    }
    let pick = match engine.pick(0, 0) {
        Ok(Some(pick)) => pick,
        Ok(None) => panic!("categorical pick resolves"),
        Err(error) => panic!("categorical pick reads: {error}"),
    };
    let super::PickEntity::VolumeSegment(segment) = pick.entity else {
        panic!("pick resolves a typed categorical entity")
    };
    assert_eq!(segment.volume, volume);
    assert_eq!(segment.label, 1);
    assert!(matches!(pick.selection, molgfx_core::AtomSelection::Empty));
}

#[test]
fn molecular_picking_remains_compatible_when_categorical_attachments_exist() {
    let source = super::tests::structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let selection = scene.add_selection(molgfx_core::AtomSelection::All);
    if let Err(error) = scene.represent(selection, molgfx_core::RepresentationKind::Spacefill) {
        panic!("spacefill represents: {error}");
    }
    let labels = match SegmentedVolume::new([2, 2, 2], Mat4::IDENTITY, Arc::from([1; 8])) {
        Ok(volume) => volume,
        Err(error) => panic!("categorical volume builds: {error}"),
    };
    let volume = scene.add_segmented_volume(labels);
    let styles = match SegmentStyleTable::new(&[SegmentStyle::new(1, Rgba8::WHITE, 1.0)]) {
        Ok(styles) => styles,
        Err(error) => panic!("styles build: {error}"),
    };
    if let Err(error) = scene.represent(volume, segmentation(styles)) {
        panic!("segmentation represents: {error}");
    }
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("mixed scene renders: {error}");
    }
    let pick = match engine.pick(0, 0) {
        Ok(Some(pick)) => pick,
        Ok(None) => panic!("molecular pick resolves"),
        Err(error) => panic!("molecular pick reads: {error}"),
    };
    assert!(matches!(pick.entity, super::PickEntity::Structure(_)));
}

fn segmentation(styles: SegmentStyleTable) -> molgfx_core::RepresentationConfig {
    Representation::segmentation().segmentation_style(SegmentationStyle {
        styles,
        ..SegmentationStyle::default()
    })
}
