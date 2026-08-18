use super::tests::{camera, engine, structure};
use pdviewx_core::{AtomSelection, RepresentationKind, Scene, TrajectoryFrame, TrajectorySegment};
use std::sync::Arc;

fn trajectory_scene() -> (Scene, pdviewx_core::StructureHandle, [usize; 2]) {
    let source = structure();
    let mut scene = Scene::new();
    let handle = scene
        .add_structure(&source)
        .unwrap_or_else(|error| panic!("{error}"));
    let selection = scene.add_selection(AtomSelection::All);
    scene
        .represent(selection, RepresentationKind::Spacefill)
        .unwrap_or_else(|error| panic!("{error}"));
    let start: Arc<[[f32; 3]]> = Arc::from([[-2.0, 0.0, 0.0], [0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
    let end: Arc<[[f32; 3]]> = Arc::from([[-2.0, 2.0, 0.0], [0.0, -2.0, 0.0], [2.0, 2.0, 0.0]]);
    let pointers = [start.as_ptr() as usize, end.as_ptr() as usize];
    let start =
        TrajectoryFrame::new(8, 0.0, start, "test:start").unwrap_or_else(|error| panic!("{error}"));
    let end =
        TrajectoryFrame::new(9, 1.0, end, "test:end").unwrap_or_else(|error| panic!("{error}"));
    let segment =
        TrajectorySegment::new(start, end, 0.25).unwrap_or_else(|error| panic!("{error}"));
    scene
        .set_trajectory_segment(handle, segment)
        .unwrap_or_else(|error| panic!("{error}"));
    (scene, handle, pointers)
}

#[test]
fn resident_frames_upload_directly_and_dispatch_once_when_the_sample_changes() {
    let (mut scene, handle, pointers) = trajectory_scene();
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(pointers.into_iter().all(|pointer| {
        writes
            .iter()
            .any(|(_, _, byte_len, source)| *byte_len == 36 && *source == pointer)
    }));
    drop(writes);
    let after_first = engine
        .device
        .log
        .dispatches
        .lock()
        .map_or_else(|error| panic!("{error}"), |values| values.len());
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let after_steady = engine
        .device
        .log
        .dispatches
        .lock()
        .map_or_else(|error| panic!("{error}"), |values| values.len());
    assert_eq!(after_steady - after_first, 2, "only culling repeats");
    scene
        .set_trajectory_time(handle, 0.75)
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let after_advance = engine
        .device
        .log
        .dispatches
        .lock()
        .map_or_else(|error| panic!("{error}"), |values| values.len());
    assert_eq!(
        after_advance - after_steady,
        3,
        "interpolation plus culling"
    );
}

#[test]
fn time_only_advances_upload_one_uniform_beside_the_frame_uniforms() {
    let (mut scene, handle, _) = trajectory_scene();
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
    scene
        .set_trajectory_time(handle, 0.5)
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        writes.len() - before,
        2,
        "new writes: {:?}",
        &writes[before..]
    );
}

#[test]
fn replacing_shared_storage_reuploads_frames_even_when_logical_ids_are_reused() {
    let (mut scene, handle, _) = trajectory_scene();
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let before = engine.device.log.writes.lock().map_or_else(
        |error| panic!("{error}"),
        |writes| writes.iter().filter(|write| write.2 == 36).count(),
    );
    let start = TrajectoryFrame::new(
        8,
        0.0,
        Arc::from([[1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]]),
        "test:replacement-start",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let end = TrajectoryFrame::new(
        9,
        1.0,
        Arc::from([[1.0, 1.0, 0.0], [2.0, 1.0, 0.0], [3.0, 1.0, 0.0]]),
        "test:replacement-end",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    scene
        .set_trajectory_segment(
            handle,
            TrajectorySegment::new(start, end, 0.5).unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let after = engine.device.log.writes.lock().map_or_else(
        |error| panic!("{error}"),
        |writes| writes.iter().filter(|write| write.2 == 36).count(),
    );
    assert_eq!(after - before, 2);
}
