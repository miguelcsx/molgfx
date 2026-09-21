use super::*;
use crate::{
    AnalyticSphere, AnalyticTemplate, AtomProperty, AtomPropertyMeaning, AttributeColumn,
    AttributeValues, BondTopologyFrame, BondTopologySegment, InstanceBatch, RigidInstance,
    RowDomain, ScalarFieldSemantics, SourceNamespace, SourceRows, TopologyBond, TrajectoryFrame,
    TrajectorySegment,
};
use molgfx_math::{Quat, Vec3};

fn warp(rate: f64, mode: PlaybackMode) -> TimeWarp {
    TimeWarp::new(0.0, 0.0, rate, [0.0, 1.0], mode).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn time_warps_clamp_loop_and_ping_pong_without_state() {
    assert_eq!(warp(1.0, PlaybackMode::Clamp).sample(1.5), Some(1.0));
    assert_eq!(warp(1.0, PlaybackMode::Loop).sample(1.25), Some(0.25));
    assert_eq!(warp(1.0, PlaybackMode::PingPong).sample(1.25), Some(0.75));
    assert_eq!(warp(-1.0, PlaybackMode::Loop).sample(0.25), Some(0.75));
}

#[test]
fn one_tick_updates_trajectory_and_scalar_tracks_at_their_own_rates() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, placed)) = scene.structures().next() else {
        panic!("owner exists")
    };
    let start_positions: Arc<[[f32; 3]]> = Arc::from(placed.atoms.coords().slice());
    let mut end_positions = start_positions.to_vec();
    for position in &mut end_positions {
        position[0] += 1.0;
    }
    let start = TrajectoryFrame::new(0, 0.0, start_positions, "start")
        .unwrap_or_else(|error| panic!("{error}"));
    let end = TrajectoryFrame::new(1, 1.0, Arc::from(end_positions), "end")
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(start, end, 0.0).unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let count = scene
        .structure(owner)
        .map_or(0, |value| value.atoms.len() as usize);
    let property = AtomProperty::new(
        owner,
        "energy",
        Arc::from(vec![0.0; count]),
        AtomPropertyMeaning::Charge,
        ScalarFieldSemantics::UncalibratedRank,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let property = scene
        .add_atom_property(property)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut timeline = Timeline::new();
    timeline
        .bind_trajectory(&scene, owner, warp(1.0, PlaybackMode::Clamp))
        .unwrap_or_else(|error| panic!("{error}"));
    timeline
        .bind_atom_property(
            &mut scene,
            property,
            Arc::from(vec![0.0; count]),
            Arc::from(vec![8.0; count]),
            warp(0.5, PlaybackMode::Clamp),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    timeline
        .apply(&mut scene, 0.5)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        scene.presentation_time_seconds().to_bits(),
        0.5f32.to_bits()
    );

    let trajectory_time = scene
        .structure(owner)
        .and_then(|value| value.trajectory())
        .map(TrajectorySegment::sample_seconds);
    assert_eq!(trajectory_time.map(f32::to_bits), Some(0.5f32.to_bits()));
    let property_value = scene
        .atom_property(property)
        .and_then(|value| value.values().first())
        .copied();
    assert_eq!(property_value.map(f32::to_bits), Some(2.0f32.to_bits()));
}

#[test]
fn a_stale_track_prevents_every_update_in_the_tick() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, placed)) = scene.structures().next() else {
        panic!("owner exists")
    };
    let positions: Arc<[[f32; 3]]> = Arc::from(placed.atoms.coords().slice());
    let count = usize::try_from(placed.atoms.len()).unwrap_or(0);
    let start = TrajectoryFrame::new(0, 0.0, Arc::clone(&positions), "start")
        .unwrap_or_else(|error| panic!("{error}"));
    let end =
        TrajectoryFrame::new(1, 1.0, positions, "end").unwrap_or_else(|error| panic!("{error}"));
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(start, end, 0.0).unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let property = AtomProperty::new(
        owner,
        "energy",
        Arc::from(vec![0.0; count]),
        AtomPropertyMeaning::Charge,
        ScalarFieldSemantics::UncalibratedRank,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let property = scene
        .add_atom_property(property)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut timeline = Timeline::new();
    timeline
        .bind_trajectory(&scene, owner, warp(1.0, PlaybackMode::Clamp))
        .unwrap_or_else(|error| panic!("{error}"));
    timeline
        .bind_atom_property(
            &mut scene,
            property,
            Arc::from(vec![0.0; count]),
            Arc::from(vec![1.0; count]),
            warp(1.0, PlaybackMode::Clamp),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    scene.remove_atom_property(property);

    assert!(matches!(
        timeline.apply(&mut scene, 0.5),
        Err(CoreError::StaleHandle)
    ));
    let time = scene
        .structure(owner)
        .and_then(|value| value.trajectory())
        .map(TrajectorySegment::sample_seconds);
    assert_eq!(time.map(f32::to_bits), Some(0.0f32.to_bits()));
    assert_eq!(
        scene.presentation_time_seconds().to_bits(),
        0.0f32.to_bits()
    );
}

#[test]
fn one_clock_drives_dynamic_connectivity_at_an_independent_rate() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let owner = scene
        .structures()
        .next()
        .map_or_else(|| panic!("owner"), |(handle, _)| handle);
    let atom_count = scene.structure(owner).map_or(0, |value| value.atoms.len());
    let from = TopologyBond::new(0, 1, false).unwrap_or_else(|error| panic!("{error}"));
    let to = TopologyBond::new(1, 2, false).unwrap_or_else(|error| panic!("{error}"));
    let start = BondTopologyFrame::new(0, 0.0, atom_count, Arc::from([from]), "start")
        .unwrap_or_else(|error| panic!("{error}"));
    let end = BondTopologyFrame::new(1, 1.0, atom_count, Arc::from([to]), "end")
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .set_bond_topology_segment(
            owner,
            BondTopologySegment::new(start, end, 0.0).unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let mut timeline = Timeline::new();
    timeline
        .bind_bond_topology(&scene, owner, warp(0.5, PlaybackMode::Clamp))
        .unwrap_or_else(|error| panic!("{error}"));
    timeline
        .apply(&mut scene, 0.5)
        .unwrap_or_else(|error| panic!("{error}"));
    let weights = scene
        .structure(owner)
        .and_then(crate::PlacedStructure::bond_topology)
        .map(|segment| {
            segment
                .bonds()
                .map(|bond| bond.weight())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    assert_eq!(weights, [0.75, 0.25]);
}

#[test]
fn generic_frames_remain_shared_and_only_their_sampling_revision_changes() {
    let mut scene = Scene::new();
    let template = Arc::new(
        AnalyticTemplate::new(
            Arc::from([AnalyticSphere {
                center: [0.0; 3],
                radius: 1.0,
            }]),
            Arc::from([]),
            SourceRows::ordered(SourceNamespace(80), 1),
        )
        .unwrap_or_else(|error| panic!("{error}")),
    );
    let source = Arc::from([RigidInstance::new(Vec3::ZERO, Quat::IDENTITY, 1.0)
        .unwrap_or_else(|error| panic!("{error}"))]);
    let target =
        Arc::from([RigidInstance::new(Vec3::X, Quat::IDENTITY, 2.0)
            .unwrap_or_else(|error| panic!("{error}"))]);
    let batch = scene.add_instance_batch(
        InstanceBatch::new(
            template,
            Arc::clone(&source),
            SourceRows::ordered(SourceNamespace(81), 1),
        )
        .unwrap_or_else(|error| panic!("{error}")),
    );
    let attribute = scene
        .add_attribute(
            AttributeColumn::new(
                RowDomain::Instances(batch),
                "weight",
                AttributeValues::Scalar(Arc::from([0.0])),
            )
            .unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let mut timeline = Timeline::new();
    let instances = timeline
        .bind_instances(
            &mut scene,
            batch,
            Arc::clone(&source),
            Arc::clone(&target),
            warp(1.0, PlaybackMode::Clamp),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    timeline
        .bind_attribute(
            &mut scene,
            attribute,
            AttributeValues::Scalar(Arc::from([2.0])),
            AttributeValues::Scalar(Arc::from([6.0])),
            warp(1.0, PlaybackMode::Clamp),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    timeline
        .apply(&mut scene, 0.25)
        .unwrap_or_else(|error| panic!("{error}"));

    let Some((resident_start, resident_end, alpha, _)) = scene.instance_frames(batch) else {
        panic!("instance frames remain resident")
    };
    assert!(Arc::ptr_eq(resident_start, &source));
    assert!(Arc::ptr_eq(resident_end, &target));
    assert_eq!(alpha.to_bits(), 0.25f32.to_bits());
    assert!(timeline.unbind(&mut scene, instances));
    assert!(scene.instance_frames(batch).is_none());
}

#[test]
fn deformable_point_frames_remain_twelve_byte_shared_rows() {
    let mut scene = Scene::new();
    let start: Arc<[[f32; 3]]> = Arc::from([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
    let end: Arc<[[f32; 3]]> = Arc::from([[0.0, 0.2, 0.0], [1.0, -0.2, 0.0]]);
    let points = scene.add_point_batch(
        crate::PointBatch::new(
            Arc::clone(&start),
            SourceRows::ordered(SourceNamespace(82), 2),
            crate::PointGlyph::Sphere,
            crate::PointStyle::default(),
        )
        .unwrap_or_else(|error| panic!("points validate: {error}")),
    );
    let mut timeline = Timeline::new();
    let track = timeline
        .bind_points(
            &mut scene,
            points,
            Arc::clone(&start),
            Arc::clone(&end),
            warp(1.0, PlaybackMode::PingPong),
        )
        .unwrap_or_else(|error| panic!("point timeline binds: {error}"));
    timeline
        .apply(&mut scene, 0.25)
        .unwrap_or_else(|error| panic!("point timeline samples: {error}"));

    let Some((resident_start, resident_end, alpha, _)) = scene.point_frames(points) else {
        panic!("point frames remain resident")
    };
    assert!(Arc::ptr_eq(resident_start, &start));
    assert!(Arc::ptr_eq(resident_end, &end));
    assert_eq!(alpha.to_bits(), 0.25f32.to_bits());
    assert_eq!(std::mem::size_of_val(resident_start.as_ref()), 24);
    assert!(timeline.unbind(&mut scene, track));
    assert!(scene.point_frames(points).is_none());
}
