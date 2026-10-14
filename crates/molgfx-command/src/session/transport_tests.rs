use super::tests::{fail, run, structure};
use crate::{Command, ErrorKind, Name, Session, TimeAmount, Transport};
use molgfx_scene::time::TimeUnit;
use molgfx_scene::timeline::{PlaybackMode, TrackPace, TrackSpec};
use molgfx_scene::{DataSource, Scene, StructureId, overlay::trajectory};
use std::collections::BTreeMap;

/// A scene with one authored track named `fast`, filling 100 ns in 10 s.
fn timed_scene() -> Scene {
    let mut scene = match Scene::from_structure(&structure()) {
        Ok(scene) => scene,
        Err(error) => panic!("scene builds: {error}"),
    };
    let id = StructureId::new(1);
    let trajectory = match scene.add(trajectory::trajectory(id, DataSource::new("t"), 2)) {
        Ok(id) => id,
        Err(error) => panic!("trajectory adds: {error}"),
    };
    let mut spec = scene.spec().clone();
    let _ = spec.timeline.tracks.insert(
        "fast".into(),
        TrackSpec {
            trajectory,
            unit: TimeUnit::Nanosecond,
            range: [0.0, 100.0],
            pace: TrackPace::Fit {
                playback_seconds: 10.0,
            },
            mode: PlaybackMode::Clamp,
        },
    );
    match Scene::from_spec(spec, BTreeMap::from([(id, structure())])) {
        Ok(scene) => scene,
        Err(error) => panic!("scene rebuilds: {error}"),
    }
}

fn amount(value: f64, unit: TimeUnit) -> TimeAmount {
    match TimeAmount::new(value, unit) {
        Ok(amount) => amount,
        Err(reason) => panic!("{reason}"),
    }
}

fn fast() -> Name {
    match Name::new("fast") {
        Ok(name) => name,
        Err(error) => panic!("a name: {error}"),
    }
}

#[test]
fn seek_moves_the_clock_without_touching_the_scene() {
    let mut scene = timed_scene();
    let mut session = Session::new(&scene);
    let revision = scene.revision();
    let outcome = run(&mut session, &mut scene, "seek 2.5 s");
    assert!(outcome.patch.is_none());
    assert_eq!(scene.revision(), revision);
    assert_eq!(session.spec().transport.time, amount(2.5, TimeUnit::Second));
}

#[test]
fn undoing_a_seek_restores_the_earlier_playback_time() {
    let mut scene = timed_scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "seek 2 s");
    run(&mut session, &mut scene, "seek 7 s");
    run(&mut session, &mut scene, "undo");
    assert_eq!(session.spec().transport.time, amount(2.0, TimeUnit::Second));
    run(&mut session, &mut scene, "undo");
    assert_eq!(session.spec().transport.time, TimeAmount::zero());
}

#[test]
fn redoing_a_seek_returns_to_the_later_time() {
    let mut scene = timed_scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "seek 2 s; seek 7 s");
    run(&mut session, &mut scene, "undo");
    assert!(session.can_redo());
    run(&mut session, &mut scene, "redo");
    assert_eq!(session.spec().transport.time, amount(7.0, TimeUnit::Second));
}

#[test]
fn dilate_sets_a_rate_and_undo_restores_the_earlier_one() {
    let mut scene = timed_scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "dilate fast 2 ns");
    run(&mut session, &mut scene, "dilate fast 9 ns");
    assert_eq!(
        session.spec().transport.dilations.get(&fast()),
        Some(&amount(9.0, TimeUnit::Nanosecond))
    );
    run(&mut session, &mut scene, "undo");
    assert_eq!(
        session.spec().transport.dilations.get(&fast()),
        Some(&amount(2.0, TimeUnit::Nanosecond))
    );
    run(&mut session, &mut scene, "undo");
    assert!(session.spec().transport.dilations.is_empty());
    run(&mut session, &mut scene, "redo");
    assert_eq!(
        session.spec().transport.dilations.get(&fast()),
        Some(&amount(2.0, TimeUnit::Nanosecond))
    );
}

#[test]
fn dilate_rejects_an_unknown_track_and_a_non_positive_rate() {
    let mut scene = timed_scene();
    let mut session = Session::new(&scene);
    let unknown = fail(&mut session, &mut scene, "dilate slow 2 ns");
    assert_eq!(unknown.kind, ErrorKind::UnknownSymbol);
    let zero = fail(&mut session, &mut scene, "dilate fast 0 ns");
    assert_eq!(zero.kind, ErrorKind::InvalidTime);
    let negative = fail(&mut session, &mut scene, "seek -1 s");
    assert_eq!(negative.kind, ErrorKind::InvalidTime);
    assert!(!session.can_undo());
}

#[test]
fn play_and_pause_change_state_but_stay_off_the_undo_stack() {
    let mut scene = timed_scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "play");
    assert!(session.is_playing());
    assert!(!session.can_undo());
    run(&mut session, &mut scene, "pause");
    assert!(!session.is_playing());
    assert!(session.history().is_empty());
}

#[test]
fn undoing_a_seek_does_not_start_or_stop_playback() {
    let mut scene = timed_scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "seek 3 s");
    run(&mut session, &mut scene, "play");
    run(&mut session, &mut scene, "undo");
    assert!(session.is_playing());
    assert_eq!(session.spec().transport.time, TimeAmount::zero());
}

#[test]
fn a_typed_command_runs_through_the_session_like_its_text() {
    let mut scene = timed_scene();
    let mut text = Session::new(&scene);
    let mut typed = Session::new(&scene);
    run(&mut text, &mut scene, "dilate fast 4 ns; seek 1 s");
    let command = |transport| Command::Transport(transport);
    for step in [
        Transport::Dilate {
            track: fast(),
            rate: amount(4.0, TimeUnit::Nanosecond),
        },
        Transport::Seek {
            time: amount(1.0, TimeUnit::Second),
        },
    ] {
        if let Err(errors) = typed.run(&mut scene, command(step)) {
            panic!("typed command runs: {errors:?}");
        }
    }
    assert_eq!(typed.spec().transport, text.spec().transport);
    assert_eq!(text.history().len(), 1);
    assert_eq!(typed.history().len(), 2);
}

#[test]
fn a_transport_state_survives_the_portable_session_spec() {
    let mut scene = timed_scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "dilate fast 4 ns; seek 1.5 s");
    let json = match session.spec().to_json() {
        Ok(json) => json,
        Err(error) => panic!("serializes: {error}"),
    };
    let spec = match crate::SessionSpec::from_json(&json) {
        Ok(spec) => spec,
        Err(error) => panic!("reads: {error}"),
    };
    assert_eq!(spec.transport, session.spec().transport);
}
