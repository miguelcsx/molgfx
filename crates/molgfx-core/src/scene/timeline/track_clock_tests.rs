use super::*;
use crate::{
    AttributeColumn, AttributeValues, PlacedStructure, RowDomain, TimeUnit, TrackClock,
    TrajectoryFrame, TrajectorySegment,
};
use std::sync::Arc;

const FS: f64 = 1.0e-15;
const NS: f64 = 1.0e-9;

fn place(scene: &mut Scene, span: f64, mode: PlaybackMode) -> (StructureHandle, TimeWarp) {
    let structure = crate::fixture::structure();
    let owner = scene
        .add_structure(&structure)
        .unwrap_or_else(|error| panic!("{error}"));
    let Some(placed) = scene.structure(owner) else {
        panic!("owner exists")
    };
    let positions: Arc<[[f32; 3]]> = Arc::from(placed.atoms.coords().slice());
    let from = TrajectoryFrame::new(0, 0.0, Arc::clone(&positions), "a")
        .unwrap_or_else(|error| panic!("{error}"));
    let to =
        TrajectoryFrame::new(1, span, positions, "b").unwrap_or_else(|error| panic!("{error}"));
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(from, to, 0.0).unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let warp = TimeWarp::fit([0.0.into(), span.into()], 10.0, mode)
        .unwrap_or_else(|error| panic!("{error}"));
    (owner, warp)
}

fn clock(scene: &Scene, owner: StructureHandle) -> TrackClock {
    scene
        .structure(owner)
        .map(PlacedStructure::track_clock)
        .unwrap_or_default()
}

fn near(left: f64, right: f64) -> bool {
    (left - right).abs() < 1e-9 * right.abs().max(1.0)
}

#[test]
fn a_warp_defaults_its_unit_to_the_one_fitting_its_interval() {
    let warp = TimeWarp::new(0.0, 0.0, 1.0, [0.0, 3.0 * NS], PlaybackMode::Clamp)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(warp.unit(), TimeUnit::Nanosecond);
}

#[test]
fn two_tracks_at_different_scales_each_read_phase_and_time_in_their_own_unit() {
    for (mode, expected) in [
        (PlaybackMode::Clamp, [0.0, 0.25, 1.0]),
        (PlaybackMode::Loop, [0.0, 0.25, 0.0]),
        (PlaybackMode::PingPong, [0.0, 0.25, 1.0]),
    ] {
        let mut scene = Scene::new();
        let (fast, fast_warp) = place(&mut scene, 100.0 * FS, mode);
        let (slow, slow_warp) = place(&mut scene, 100.0 * NS, mode);
        let mut timeline = Timeline::new();
        timeline
            .bind_trajectory(&scene, fast, fast_warp)
            .unwrap_or_else(|error| panic!("{error}"));
        timeline
            .bind_trajectory(&scene, slow, slow_warp)
            .unwrap_or_else(|error| panic!("{error}"));
        for (playback, phase) in [0.0_f64, 2.5, 10.0].into_iter().zip(expected) {
            timeline
                .apply(&mut scene, playback)
                .unwrap_or_else(|error| panic!("{error}"));
            let fast_clock = clock(&scene, fast);
            let slow_clock = clock(&scene, slow);
            // A loop at exactly one period sits on the wrap, where either end is right.
            let wrapped = mode == PlaybackMode::Loop && playback.to_bits() == 10.0_f64.to_bits();
            for (reading, unit) in [(fast_clock, "fs"), (slow_clock, "ns")] {
                let ends = [phase, 1.0 - phase];
                let wanted = if wrapped { &ends[..] } else { &ends[..1] };
                assert!(
                    wanted
                        .iter()
                        .any(|end| near(reading.phase, *end)
                            && near(reading.display_time, *end * 100.0)),
                    "{unit} {mode:?} {playback}"
                );
            }
        }
    }
}

#[test]
fn a_track_clock_change_bumps_the_presentation_revision_and_a_repeat_does_not() {
    let mut scene = Scene::new();
    let (owner, warp) = place(&mut scene, 100.0 * FS, PlaybackMode::Clamp);
    let mut timeline = Timeline::new();
    timeline
        .bind_trajectory(&scene, owner, warp)
        .unwrap_or_else(|error| panic!("{error}"));
    timeline
        .apply(&mut scene, 1.0)
        .unwrap_or_else(|error| panic!("{error}"));
    let revision = scene.presentation_revision();
    timeline
        .apply(&mut scene, 1.0)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.presentation_revision(), revision);
    timeline
        .apply(&mut scene, 2.0)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_ne!(scene.presentation_revision(), revision);
}

#[test]
fn attribute_tracks_cross_fade_a_bound_column_with_a_morph() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, placed)) = scene.structures().next() else {
        panic!("owner exists")
    };
    let count = usize::try_from(placed.atoms.len()).unwrap_or(0);
    let positions: Arc<[[f32; 3]]> = Arc::from(placed.atoms.coords().slice());
    let from = TrajectoryFrame::new(0, 0.0, Arc::clone(&positions), "a")
        .unwrap_or_else(|error| panic!("{error}"));
    let to = TrajectoryFrame::new(1, 1.0, positions, "b").unwrap_or_else(|error| panic!("{error}"));
    let opacity = scene
        .add_attribute(
            AttributeColumn::new(
                RowDomain::Atoms(owner),
                "opacity",
                AttributeValues::Scalar(Arc::from(vec![0.0; count])),
            )
            .unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let mut timeline = Timeline::new();
    timeline
        .morph(&mut scene, owner, from, to, 2.0)
        .unwrap_or_else(|error| panic!("{error}"));
    let fade = TimeWarp::fit([0.0.into(), 1.0.into()], 2.0, PlaybackMode::Clamp)
        .unwrap_or_else(|error| panic!("{error}"));
    timeline
        .bind_attribute(
            &mut scene,
            opacity,
            AttributeValues::Scalar(Arc::from(vec![0.0; count])),
            AttributeValues::Scalar(Arc::from(vec![1.0; count])),
            fade,
        )
        .unwrap_or_else(|error| panic!("{error}"));
    timeline
        .apply(&mut scene, 0.5)
        .unwrap_or_else(|error| panic!("{error}"));
    let Some((start, end, alpha, _)) = scene.attribute_frames(opacity) else {
        panic!("the cross-fade binds a resident pair")
    };
    assert_eq!(alpha.to_bits(), 0.25f32.to_bits());
    assert_eq!(start, &AttributeValues::Scalar(Arc::from(vec![0.0; count])));
    assert_eq!(end, &AttributeValues::Scalar(Arc::from(vec![1.0; count])));
    let Some(segment) = scene.structure(owner).and_then(|value| value.trajectory()) else {
        panic!("the morph installs a segment")
    };
    assert_eq!(segment.interpolation().to_bits(), 0.25f64.to_bits());
}
