use super::*;
use crate::{
    AtomProperty, AtomPropertyMeaning, PlaybackMode, ScalarFieldSemantics, Scene, TimeUnit,
    TimeWarp,
};
use std::sync::Arc;

fn warp(range: [f64; 2], unit: TimeUnit) -> TimeWarp {
    let range = range.map(|value| SimTime::from_unit(value, unit).expect("finite time"));
    TimeWarp::fit(range, 10.0, PlaybackMode::Clamp).expect("valid warp")
}

#[test]
fn each_track_reads_what_its_warp_samples_at_every_playback_time() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).expect("scene");
    let Some((owner, placed)) = scene.structures().next() else {
        panic!("owner exists")
    };
    let count = usize::try_from(placed.atoms.len()).unwrap_or(0);
    let mut timeline = Timeline::new();
    let warps = [
        warp([0.0, 100.0], TimeUnit::Femtosecond),
        warp([0.0, 50.0], TimeUnit::Nanosecond),
    ];
    let mut handles = Vec::new();
    for (index, warp) in warps.into_iter().enumerate() {
        let property = AtomProperty::new(
            owner,
            format!("track-{index}"),
            Arc::from(vec![0.0; count]),
            AtomPropertyMeaning::Charge,
            ScalarFieldSemantics::UncalibratedRank,
        )
        .expect("property");
        let property = scene.add_atom_property(property).expect("added");
        handles.push(
            timeline
                .bind_atom_property(
                    &mut scene,
                    property,
                    Arc::from(vec![0.0; count]),
                    Arc::from(vec![1.0; count]),
                    warp,
                )
                .expect("bound"),
        );
    }
    for playback in [0.0, 2.5, 10.0] {
        timeline.apply(&mut scene, playback).expect("applied");
        assert!((timeline.playback_time().seconds() - playback).abs() < f64::EPSILON);
        for (handle, warp) in handles.iter().zip(warps) {
            let reading = timeline.reading(*handle).expect("reading");
            assert_eq!(Some(reading.time.seconds()), warp.sample(playback));
            assert_eq!(Some(reading.phase), warp.phase(playback));
            assert_eq!(reading.unit, warp.unit());
        }
        assert_eq!(timeline.readings().count(), 2);
    }
    let [fast, slow] = handles[..] else {
        panic!("two tracks")
    };
    assert_eq!(
        timeline.reading(fast).map(|r| r.unit),
        Some(TimeUnit::Femtosecond)
    );
    let drift = timeline.reading(slow).expect("reading");
    assert_eq!(drift.unit, TimeUnit::Nanosecond);
    assert!((drift.display_value() - 50.0).abs() < 1e-9);
}

#[test]
fn an_empty_timeline_has_no_readings() {
    let timeline = Timeline::new();
    assert!(timeline.readings().next().is_none());
    assert!(timeline.track_handles().next().is_none());
}
