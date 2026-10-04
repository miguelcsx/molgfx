use super::tests::{camera, engine};
use crate::testing::{MockDevice, MockLog};
use molgfx_core::{
    Representation, RepresentationHandle, Scene, SegmentStyle, SegmentStyleTable,
    SegmentationStyle, SegmentedVolume,
};
use molgfx_math::{Mat4, Rgba8};
use std::sync::Arc;

fn styles(label: u32, color: Rgba8, opacity: f32) -> SegmentStyleTable {
    SegmentStyleTable::new(&[SegmentStyle::new(label, color, opacity)]).expect("styles validate")
}

fn scene(table: SegmentStyleTable) -> (Scene, RepresentationHandle) {
    let mut scene = Scene::new();
    let labels = SegmentedVolume::new([2; 3], Mat4::IDENTITY, Arc::from([1, 2, 1, 2, 0, 1, 2, 0]))
        .expect("label grid validates");
    let volume = scene.add_segmented_volume(labels);
    let representation = scene
        .represent(
            volume,
            Representation::segmentation().segmentation_style(SegmentationStyle {
                styles: table,
                ..SegmentationStyle::default()
            }),
        )
        .expect("segmentation represents");
    (scene, representation)
}

fn render(engine: &mut super::Engine<MockDevice>, scene: &Scene) {
    engine
        .render(scene, &camera())
        .expect("segmentation renders");
}

fn buffer_writes(log: &MockLog, label: &str) -> usize {
    let buffers = log.buffers.lock().expect("buffer log lock");
    let writes = log.writes.lock().expect("upload log lock");
    writes
        .iter()
        .filter(|write| {
            buffers
                .iter()
                .any(|buffer| buffer.0 == write.0 && buffer.1 == label)
        })
        .count()
}

fn label_uploads(log: &MockLog) -> usize {
    log.texture_writes
        .lock()
        .expect("texture upload log lock")
        .iter()
        .filter(|write| write.0 == "caller categorical segmentation")
        .count()
}

#[test]
fn changing_segment_color_and_opacity_uploads_only_the_style_table() {
    for label in [1, u32::MAX] {
        let (mut scene, representation) = scene(styles(label, Rgba8::WHITE, 0.8));
        let mut engine = engine();
        render(&mut engine, &scene);
        let initial_styles = buffer_writes(&engine.device.log, "categorical segment style lookup");
        let initial_uniforms =
            buffer_writes(&engine.device.log, "categorical segmentation uniforms");
        let initial_buffers = engine
            .device
            .log
            .buffers
            .lock()
            .expect("buffer log lock")
            .len();
        scene
            .representation_mut(representation)
            .expect("representation exists")
            .segmentation
            .styles = styles(label, Rgba8::opaque(220, 30, 80), 0.25);
        render(&mut engine, &scene);
        assert_eq!(label_uploads(&engine.device.log), 1);
        assert_eq!(
            buffer_writes(&engine.device.log, "categorical segment style lookup"),
            initial_styles + 1
        );
        assert_eq!(
            buffer_writes(&engine.device.log, "categorical segmentation uniforms"),
            initial_uniforms
        );
        assert_eq!(
            engine
                .device
                .log
                .buffers
                .lock()
                .expect("buffer log lock")
                .len(),
            initial_buffers
        );
        render(&mut engine, &scene);
        assert_eq!(
            buffer_writes(&engine.device.log, "categorical segment style lookup"),
            initial_styles + 1
        );
    }
}

#[test]
fn sampling_edits_do_not_reupload_the_style_table_or_labels() {
    let (mut scene, representation) = scene(styles(1, Rgba8::WHITE, 0.8));
    let mut engine = engine();
    render(&mut engine, &scene);
    let initial_styles = buffer_writes(&engine.device.log, "categorical segment style lookup");
    let initial_uniforms = buffer_writes(&engine.device.log, "categorical segmentation uniforms");
    scene
        .representation_mut(representation)
        .expect("representation exists")
        .segmentation
        .opacity_scale = 0.25;
    render(&mut engine, &scene);
    assert_eq!(label_uploads(&engine.device.log), 1);
    assert_eq!(
        buffer_writes(&engine.device.log, "categorical segment style lookup"),
        initial_styles
    );
    assert_eq!(
        buffer_writes(&engine.device.log, "categorical segmentation uniforms"),
        initial_uniforms + 1
    );
}

#[test]
fn empty_styles_remove_all_drawables_and_restoring_styles_keeps_labels_resident() {
    let (mut scene, representation) = scene(styles(1, Rgba8::WHITE, 0.8));
    let mut engine = engine();
    render(&mut engine, &scene);
    assert_eq!(engine.scene_gpu.segmentation_draws().count(), 1);
    {
        let representation = scene
            .representation_mut(representation)
            .expect("representation exists");
        representation.segmentation.styles = SegmentStyleTable::default();
        representation.visible = false;
    }
    render(&mut engine, &scene);
    assert_eq!(engine.scene_gpu.segmentation_draws().count(), 0);
    assert!(!engine.scene_gpu.has_translucency());
    assert_eq!(label_uploads(&engine.device.log), 1);
    {
        let representation = scene
            .representation_mut(representation)
            .expect("representation exists");
        representation.segmentation.styles = styles(1, Rgba8::WHITE, 0.8);
        representation.visible = true;
    }
    render(&mut engine, &scene);
    assert_eq!(engine.scene_gpu.segmentation_draws().count(), 1);
    assert_eq!(label_uploads(&engine.device.log), 1);
}

#[test]
fn an_initially_empty_style_table_has_no_drawables() {
    let (scene, _) = scene(SegmentStyleTable::default());
    let mut engine = engine();
    render(&mut engine, &scene);
    assert_eq!(engine.scene_gpu.segmentation_draws().count(), 0);
    assert!(!engine.scene_gpu.has_translucency());
}

#[test]
fn surface_style_edits_reuse_label_and_boundary_uploads() {
    let (mut scene, representation) = scene(styles(1, Rgba8::WHITE, 0.8));
    scene
        .representation_mut(representation)
        .unwrap()
        .segmentation
        .presentation = molgfx_core::SegmentationPresentation::Surface;
    let mut engine = engine();
    render(&mut engine, &scene);
    let vertices = buffer_writes(&engine.device.log, "categorical boundary vertices");
    let indices = buffer_writes(&engine.device.log, "categorical boundary indices");
    assert_eq!(vertices, 1);
    assert_eq!(indices, 1);
    scene
        .representation_mut(representation)
        .unwrap()
        .segmentation
        .styles = styles(1, Rgba8::opaque(30, 90, 210), 0.4);
    render(&mut engine, &scene);
    assert_eq!(label_uploads(&engine.device.log), 1);
    assert_eq!(
        buffer_writes(&engine.device.log, "categorical boundary vertices"),
        vertices
    );
    assert_eq!(
        buffer_writes(&engine.device.log, "categorical boundary indices"),
        indices
    );
    assert_eq!(engine.scene_gpu.segmentation_draws().count(), 1);
}
