use super::{PickStream, PickUpdate};
use crate::engine::integration::tests::{engine, render_ok, represented_scene, structure};
use molgfx_core::PickingError;
use std::sync::atomic::Ordering;

type TestEngine = crate::Engine<crate::testing::MockDevice>;

fn hold(engine: &TestEngine, held: bool) {
    engine.device.log.hold_maps.store(held, Ordering::Release);
}

fn taken(engine: &mut TestEngine) -> Vec<PickUpdate> {
    engine.take_picks().collect()
}

fn polled(engine: &mut TestEngine) -> usize {
    match engine.poll() {
        Ok(report) => report.picks_resolved,
        Err(error) => panic!("poll succeeds: {error}"),
    }
}

fn asked(engine: &mut TestEngine, stream: PickStream, x: u32, y: u32) {
    if let Err(error) = engine.request_pick(stream, x, y) {
        panic!("request is accepted: {error}");
    }
}

#[test]
fn a_request_resolves_only_after_the_map_completes_on_a_poll() {
    let mut engine = engine();
    render_ok(&mut engine, &represented_scene(2, 1));
    hold(&engine, true);
    asked(&mut engine, PickStream::Hover, 0, 0);
    assert_eq!(polled(&mut engine), 0);
    assert!(taken(&mut engine).is_empty());
    hold(&engine, false);
    assert_eq!(polled(&mut engine), 1);
    let updates = taken(&mut engine);
    assert_eq!(updates.len(), 1);
    assert!(matches!(updates[0].pick, Ok(Some(_))));
}

#[test]
fn a_hover_stream_keeps_only_its_newest_request() {
    let mut engine = engine();
    render_ok(&mut engine, &represented_scene(2, 1));
    hold(&engine, true);
    asked(&mut engine, PickStream::Hover, 1, 0);
    polled(&mut engine);
    asked(&mut engine, PickStream::Hover, 2, 0);
    asked(&mut engine, PickStream::Hover, 3, 0);
    polled(&mut engine);
    hold(&engine, false);
    polled(&mut engine);
    let updates = taken(&mut engine);
    assert_eq!(updates.len(), 1);
    assert_eq!((updates[0].x, updates[0].stream), (3, PickStream::Hover));
}

#[test]
fn an_activation_is_never_superseded_by_hover() {
    let mut engine = engine();
    render_ok(&mut engine, &represented_scene(2, 1));
    hold(&engine, true);
    asked(&mut engine, PickStream::Activate, 1, 0);
    asked(&mut engine, PickStream::Hover, 5, 0);
    asked(&mut engine, PickStream::Activate, 2, 0);
    polled(&mut engine);
    hold(&engine, false);
    polled(&mut engine);
    let updates = taken(&mut engine);
    let activations: Vec<u32> = updates
        .iter()
        .filter(|update| update.stream == PickStream::Activate)
        .map(|update| update.x)
        .collect();
    assert_eq!(activations, [1, 2]);
    assert_eq!(updates.len(), 3);
}

#[test]
fn a_full_ordered_stream_refuses_instead_of_growing() {
    let mut engine = engine();
    render_ok(&mut engine, &represented_scene(1, 1));
    for x in 0..4 {
        asked(&mut engine, PickStream::Activate, x, 0);
    }
    let Err(error) = engine.request_pick(PickStream::Activate, 4, 0) else {
        panic!("a fifth queued activation is refused")
    };
    assert!(matches!(
        error,
        crate::RenderError::Picking(PickingError::StreamBusy)
    ));
}

#[test]
fn a_pixel_outside_the_target_resolves_to_nothing() {
    let mut engine = engine();
    render_ok(&mut engine, &represented_scene(1, 1));
    let outside = engine.width;
    asked(&mut engine, PickStream::Hover, outside, 0);
    assert_eq!(polled(&mut engine), 1);
    assert!(matches!(taken(&mut engine)[0].pick, Ok(None)));
}

#[test]
fn resizing_drops_requests_that_name_the_old_size() {
    let mut engine = engine();
    render_ok(&mut engine, &represented_scene(1, 1));
    asked(&mut engine, PickStream::Hover, 0, 0);
    asked(&mut engine, PickStream::Activate, 0, 0);
    engine.resize(32, 32);
    assert_eq!(polled(&mut engine), 0);
    assert!(taken(&mut engine).is_empty());
}

#[test]
fn a_pick_in_flight_resolves_against_the_pages_it_was_recorded_with() {
    let mut engine = engine();
    let mut scene = represented_scene(2, 1);
    render_ok(&mut engine, &scene);
    hold(&engine, true);
    asked(&mut engine, PickStream::Hover, 0, 0);
    polled(&mut engine);

    let source = structure();
    let asset = molgfx_core::StructureAsset::new(molgfx_core::DatasetId::new(99), &source)
        .unwrap_or_else(|error| panic!("{error}"));
    scene.add_asset(&asset);
    render_ok(&mut engine, &scene);

    hold(&engine, false);
    polled(&mut engine);
    let updates = taken(&mut engine);
    assert!(matches!(
        updates[0].pick,
        Err(crate::RenderError::Picking(PickingError::StaleGeneration))
    ));
}

#[test]
fn slots_free_up_so_a_long_run_of_requests_keeps_resolving() {
    let mut engine = engine();
    render_ok(&mut engine, &represented_scene(2, 1));
    let mut resolved = 0;
    for round in 0..64 {
        asked(&mut engine, PickStream::Hover, round % 4, 0);
        polled(&mut engine);
        resolved += taken(&mut engine).len();
    }
    assert!(resolved >= 32);
}
