//! Schedule validation and manifest contents, none of which needs a device.

use super::{CAPTION, FrameSchedule, SequenceManifest};
use crate::Error;
use crate::render::timeline_fixture_tests::morphing_scene;

#[test]
fn a_non_finite_or_non_positive_frame_rate_is_a_typed_error() {
    for fps in [0.0, -24.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let schedule = FrameSchedule::converged(fps, 0..4);
        assert!(
            matches!(schedule.validate(), Err(Error::InvalidSpec(_))),
            "fps {fps}"
        );
    }
}

#[test]
fn zero_frames_is_a_typed_error() {
    for (start, end) in [(0, 0), (5, 5), (7, 3)] {
        let schedule = FrameSchedule::converged(24.0, std::ops::Range { start, end });
        assert!(matches!(schedule.validate(), Err(Error::InvalidSpec(_))));
    }
}

#[test]
fn a_manifest_refuses_an_invalid_schedule() {
    let (_, timeline) = morphing_scene();
    let schedule = FrameSchedule::converged(f64::NAN, 0..4);
    assert!(SequenceManifest::new(&schedule, &timeline).is_err());
}

#[test]
fn frame_times_are_the_exact_quotient_of_index_and_rate() {
    let schedule = FrameSchedule::converged(30.0, 0..100);
    for index in [0_u32, 1, 3, 29, 97] {
        let seconds = schedule
            .playback_seconds(u64::from(index))
            .unwrap_or_else(|error| panic!("{error}"));
        let expected = f64::from(index) / 30.0;
        assert_eq!(seconds.to_bits(), expected.to_bits());
    }
}

#[test]
fn manifest_times_equal_the_frame_quotient_and_each_tracks_mapping() {
    let (mut scene, mut timeline) = morphing_scene();
    let schedule = FrameSchedule::converged(8.0, 2..7);
    let mut manifest =
        SequenceManifest::new(&schedule, &timeline).unwrap_or_else(|error| panic!("{error}"));
    let handle = timeline
        .track_handles()
        .next()
        .unwrap_or_else(|| panic!("one track"));
    let Some(warp) = timeline.warp(handle) else {
        panic!("warp")
    };
    for index in schedule.range.clone() {
        let playback = schedule
            .playback_seconds(index)
            .unwrap_or_else(|error| panic!("{error}"));
        scene
            .apply_timeline(&mut timeline, playback)
            .unwrap_or_else(|error| panic!("{error}"));
        manifest
            .record(&timeline)
            .unwrap_or_else(|error| panic!("{error}"));
    }
    assert_eq!(manifest.len(), 5);
    for (row, index) in (2_u32..7).enumerate() {
        let expected = f64::from(index) / 8.0;
        assert_eq!(
            manifest.playback_seconds()[row].to_bits(),
            expected.to_bits()
        );
        let mapped = warp
            .sample(expected)
            .unwrap_or_else(|| panic!("mapping at {expected}"));
        let recorded = manifest
            .sim_seconds(row, 0)
            .unwrap_or_else(|| panic!("recorded"));
        assert_eq!(recorded.to_bits(), mapped.to_bits());
    }
    assert_eq!(manifest.sim_seconds(0, 1), None);
}

#[test]
fn the_json_manifest_carries_frames_tracks_schedule_and_caption_without_a_version() {
    let (mut scene, mut timeline) = morphing_scene();
    let schedule = FrameSchedule::converged(4.0, 0..3);
    let mut manifest =
        SequenceManifest::new(&schedule, &timeline).unwrap_or_else(|error| panic!("{error}"));
    for index in schedule.range.clone() {
        let playback = schedule
            .playback_seconds(index)
            .unwrap_or_else(|error| panic!("{error}"));
        scene
            .apply_timeline(&mut timeline, playback)
            .unwrap_or_else(|error| panic!("{error}"));
        manifest
            .record(&timeline)
            .unwrap_or_else(|error| panic!("{error}"));
    }
    let json = manifest.to_json();
    assert_eq!(json["caption"], CAPTION);
    assert_eq!(json["schedule"]["fps"], 4.0);
    assert_eq!(json["schedule"]["output"], "converged");
    assert_eq!(json["frames"].as_array().map(Vec::len), Some(3));
    assert_eq!(json["frames"][2]["frame"], 2);
    assert_eq!(json["frames"][2]["playback_seconds"], 0.5);
    assert!(json["frames"][1]["tracks"][0]["unit"].is_string());
    assert!(json["tracks"][0]["dilation_sim_seconds_per_playback_second"].is_number());
    assert!(json.get("version").is_none());
}

#[test]
fn consecutive_manifests_join_and_a_gap_is_refused() {
    let (mut scene, mut timeline) = morphing_scene();
    let piece = |range: std::ops::Range<u64>, scene: &mut crate::Scene, timeline: &mut _| {
        let schedule = FrameSchedule::converged(4.0, range);
        let mut manifest =
            SequenceManifest::new(&schedule, timeline).unwrap_or_else(|error| panic!("{error}"));
        for index in schedule.range.clone() {
            let playback = schedule
                .playback_seconds(index)
                .unwrap_or_else(|error| panic!("{error}"));
            scene
                .apply_timeline(timeline, playback)
                .unwrap_or_else(|error| panic!("{error}"));
            manifest
                .record(timeline)
                .unwrap_or_else(|error| panic!("{error}"));
        }
        manifest
    };
    let mut joined = piece(0..2, &mut scene, &mut timeline);
    let gap = piece(3..4, &mut scene, &mut timeline);
    assert!(joined.append(&gap).is_err());
    let next = piece(2..4, &mut scene, &mut timeline);
    joined
        .append(&next)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(joined.len(), 4);
    assert_eq!(joined.to_json()["frames"][3]["frame"], 3);
}
