use super::tests::{camera, engine};
use pdviewx_core::{
    ClipPlane, DensityVolume, Representation, Scene, SegmentStyle, SegmentStyleTable,
    SegmentationStyle, SegmentedVolume, VolumeSlice, VolumeStyle,
};
use pdviewx_math::{Mat4, Rgba8, Vec3};
use std::sync::Arc;

#[test]
fn density_volume_uploads_the_callers_values_once_and_draws_through_oit() {
    let values: Arc<[f32]> = (0..64)
        .map(|index| f32::from(u8::try_from(index).map_or(u8::MAX, |value| value)) / 63.0)
        .collect();
    let source_pointer = values.as_ptr() as usize;
    let volume = match DensityVolume::from_spacing([4, 4, 4], Vec3::splat(-1.5), Vec3::ONE, values)
    {
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
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("volume renders: {error}")
    }
    let uploads = match engine.device.log.texture_writes.lock() {
        Ok(uploads) => uploads.clone(),
        Err(error) => panic!("log lock: {error}"),
    };
    assert_eq!(uploads.len(), 3);
    assert_eq!(uploads[0], ("caller density volume", 256, source_pointer));
    assert_eq!(uploads[1].0, "density empty-space minimum");
    assert_eq!(uploads[2].0, "density empty-space maximum");
    assert_eq!(engine.scene_gpu.volume_draws().count(), 3);
    assert!(engine.scene_gpu.has_translucency());
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("unchanged volume renders: {error}")
    }
    let Ok(uploads) = engine.device.log.texture_writes.lock() else {
        panic!("log lock")
    };
    assert_eq!(uploads.len(), 3, "representations share one resident grid");
}

#[test]
fn independent_scalar_channels_keep_distinct_residency_and_styles() {
    let first_values: Arc<[f32]> = Arc::from([0.1; 8]);
    let second_values: Arc<[f32]> = Arc::from([0.9; 8]);
    let pointers = [
        first_values.as_ptr() as usize,
        second_values.as_ptr() as usize,
    ];
    let first = match DensityVolume::new([2, 2, 2], Mat4::IDENTITY, first_values) {
        Ok(volume) => volume,
        Err(error) => panic!("first channel builds: {error}"),
    };
    let second = match DensityVolume::new([2, 2, 2], Mat4::IDENTITY, second_values) {
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
    assert_eq!(uploads.len(), 6);
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
        match SegmentStyleTable::new(&[SegmentStyle::new(2, Rgba8::opaque(30, 80, 220), 0.6)]) {
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
    assert_eq!(engine.scene_gpu.segmentation_draws().count(), 2);
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
    assert!(matches!(pick.selection, pdviewx_core::AtomSelection::Empty));
}

#[test]
fn molecular_picking_remains_compatible_when_categorical_attachments_exist() {
    let source = super::tests::structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let selection = scene.add_selection(pdviewx_core::AtomSelection::All);
    if let Err(error) = scene.represent(selection, pdviewx_core::RepresentationKind::Spacefill) {
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

fn segmentation(styles: SegmentStyleTable) -> pdviewx_core::RepresentationConfig {
    Representation::segmentation().segmentation_style(SegmentationStyle {
        styles,
        ..SegmentationStyle::default()
    })
}
