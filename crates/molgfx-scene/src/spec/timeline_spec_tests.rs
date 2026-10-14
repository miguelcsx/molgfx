use super::*;
use crate::overlay::trajectory;
use crate::{DataSource, Scene, SceneSpec, StructureId, TrajectoryFrame};
use molgfx_core::Timeline;
use std::sync::Arc;

fn structure() -> molframe::Structure {
    molframe::read_bytes(
        b"HETATM    1  O   HOH A   1       0.000   0.000   0.000  1.00 10.00           O\nEND\n"
            .to_vec(),
        Some("timeline.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source")
    .0
}

fn track(trajectory: TrajectoryId, unit: TimeUnit, end: f64) -> TrackSpec {
    TrackSpec {
        trajectory,
        unit,
        range: [0.0, end],
        pace: TrackPace::Fit {
            playback_seconds: 10.0,
        },
        mode: PlaybackMode::Clamp,
    }
}

fn two_track_scene() -> (Scene, TimelineSpec) {
    let mut scene = Scene::from_structure(&structure()).expect("scene");
    let id = StructureId::new(1);
    let source = DataSource::new("timeline-trajectory");
    let first = scene
        .add(trajectory::trajectory(id, source.clone(), 2))
        .expect("first trajectory");
    let second = scene
        .add(trajectory::trajectory(id, source, 2))
        .expect("second trajectory");
    scene
        .set_trajectory_pair(
            id,
            TrajectoryFrame::new(0, 0.0, Arc::from([[0.0; 3]])),
            TrajectoryFrame::new(1, 1.0e-6, Arc::from([[1.0, 0.0, 0.0]])),
            0.0,
        )
        .expect("pair");
    let mut spec = TimelineSpec::default();
    spec.tracks
        .insert("fast".into(), track(first, TimeUnit::Nanosecond, 100.0));
    spec.tracks
        .insert("slow".into(), track(second, TimeUnit::Microsecond, 1.0));
    (scene, spec)
}

#[test]
fn a_two_track_spec_lowers_to_warps_that_read_in_their_own_units() {
    let (_, spec) = two_track_scene();
    let fast = spec.tracks["fast"].warp().expect("fast warp");
    let slow = spec.tracks["slow"].warp().expect("slow warp");
    assert_eq!(fast.unit(), TimeUnit::Nanosecond);
    assert_eq!(slow.unit(), TimeUnit::Microsecond);
    assert!((fast.range()[1] - 100.0e-9).abs() < 1e-21);
    assert!((slow.range()[1] - 1.0e-6).abs() < 1e-18);
    let halfway = fast.sample(5.0).expect("sample");
    assert!((halfway - 50.0e-9).abs() < 1e-21, "{halfway}");
    let halfway = slow.sample(5.0).expect("sample");
    assert!((halfway - 0.5e-6).abs() < 1e-18, "{halfway}");
    let (fast_phase, slow_phase) = (
        fast.phase(5.0).expect("phase"),
        slow.phase(5.0).expect("phase"),
    );
    assert!((fast_phase - 0.5).abs() < 1e-9 && (slow_phase - 0.5).abs() < 1e-9);
}

#[test]
fn a_dilation_track_advances_at_its_authored_pace() {
    let (_, spec) = two_track_scene();
    let mut paced = spec.tracks["fast"];
    paced.pace = TrackPace::Dilation {
        per_playback_second: 10.0,
    };
    let sampled = paced.warp().expect("warp").sample(2.0).expect("sample");
    assert!((sampled - 20.0e-9).abs() < 1e-21, "{sampled}");
}

#[test]
fn a_spec_binds_to_a_core_timeline_that_drives_the_scene() {
    let (mut scene, spec) = two_track_scene();
    let mut document = scene.spec().clone();
    document.timeline = spec;
    document.timeline.tracks.remove("slow");
    let id = StructureId::new(1);
    let structures = std::collections::BTreeMap::from([(id, structure())]);
    scene = Scene::from_spec(document, structures).expect("scene with timeline");
    scene
        .set_trajectory_pair(
            id,
            TrajectoryFrame::new(0, 0.0, Arc::from([[0.0; 3]])),
            TrajectoryFrame::new(1, 1.0e-6, Arc::from([[1.0, 0.0, 0.0]])),
            0.0,
        )
        .expect("pair");
    let mut timeline: Timeline = scene.lower_timeline().expect("lowered");
    assert_eq!(timeline.len(), 1);
    scene.apply_timeline(&mut timeline, 5.0).expect("applied");
    let readings: Vec<_> = timeline.readings().collect();
    assert_eq!(readings.len(), 1);
    assert!(
        readings
            .iter()
            .all(|(_, reading)| (reading.phase - 0.5).abs() < 1e-9)
    );
}

#[test]
fn an_unknown_unit_string_fails_to_parse() {
    let (_, spec) = two_track_scene();
    let Ok(json) = serde_json::to_string(&spec) else {
        panic!("serializable");
    };
    assert!(json.contains("\"ns\""));
    let round: TimelineSpec = serde_json::from_str(&json).expect("round trip");
    assert_eq!(round, spec);
    let bad = json.replace("\"ns\"", "\"fortnights\"");
    let failure = serde_json::from_str::<TimelineSpec>(&bad).expect_err("unknown unit");
    assert!(
        failure.to_string().contains("unknown time unit"),
        "{failure}"
    );
}

#[test]
fn a_track_over_an_undeclared_trajectory_is_rejected() {
    let (_, spec) = two_track_scene();
    let declared = SceneSpec::empty().trajectories;
    assert!(spec.validate(&declared).is_err());
}

#[test]
fn a_track_with_an_empty_interval_is_rejected() {
    let (_, spec) = two_track_scene();
    let mut empty = spec.tracks["fast"];
    empty.range = [5.0, 5.0];
    assert!(empty.warp().is_err());
}
