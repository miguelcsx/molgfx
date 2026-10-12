//! Instantaneous source-local distance and explicit-pivot angle gates on a GPU.
#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{
    OrientedGeometry, OrientedRelation, Relation, RelationBatch, RelationPattern, RelationStyle,
    RowDomain, RowEntityRef, Scene, SourceNamespace, SourceRows, SpatialAnchor, StructureHandle,
    TrajectoryFrame, TrajectorySegment,
};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn pixels(engine: &mut Engine<WgpuDevice>, scene: &Scene) -> Vec<u8> {
    engine
        .render_image(
            scene,
            &Camera {
                eye: Vec3::new(1.5, 0.0, 20.0),
                target: Vec3::new(1.5, 0.0, 0.0),
                up: Vec3::Y,
                projection: Projection::Orthographic {
                    height: 10.0,
                    aspect: 1.0,
                    near: 0.1,
                    far: 40.0,
                },
            },
            ImageConfig {
                width: 256,
                height: 256,
            },
        )
        .expect("oriented relation render")
        .pixels
}

fn coverage(image: &[u8]) -> usize {
    image
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] > 0)
        .count()
}

fn segment(
    scene: &mut Scene,
    owner: StructureHandle,
    first: [[f32; 3]; 3],
    last: [[f32; 3]; 3],
    identity: u64,
) {
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(
                TrajectoryFrame::new(identity, 0.0, Arc::from(first), format!("first-{identity}"))
                    .expect("first"),
                TrajectoryFrame::new(
                    identity + 1,
                    1.0,
                    Arc::from(last),
                    format!("last-{identity}"),
                )
                .expect("last"),
                0.0,
            )
            .expect("segment"),
        )
        .expect("resident coordinates");
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn oriented_connectors_appear_by_distance_and_disappear_by_angle_without_rebinding_rows() {
    for angle in [150.0, 180.0] {
        verify_geometry(angle);
    }
}

fn verify_geometry(minimum_angle_degrees: f32) {
    let mut scene = fixture_scene();
    let owner = scene.structures().next().expect("structure").0;
    let domain = RowDomain::Atoms(owner);
    scene
        .add_relation_batch(
            RelationBatch::new(
                Arc::from([Relation {
                    start: SpatialAnchor::entity(RowEntityRef::new(domain, 0)).expect("donor"),
                    end: SpatialAnchor::entity(RowEntityRef::new(domain, 2)).expect("acceptor"),
                }]),
                SourceRows::ordered(SourceNamespace(1), 1),
                RelationStyle {
                    width_pixels: 4.0,
                    pattern: RelationPattern::Dotted,
                    ..RelationStyle::default()
                },
            )
            .expect("relations")
            .with_oriented_geometry(Arc::from([OrientedRelation {
                pivot: RowEntityRef::new(domain, 1),
                geometry: OrientedGeometry {
                    maximum_endpoint_distance: 3.5,
                    minimum_angle_degrees,
                    maximum_pivot_end_distance: Some(2.5),
                },
            }]))
            .expect("oriented geometry"),
        )
        .expect("batch");
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            profile: RenderProfile::bare()
                .with_effect(PresentationEffect::Backdrop(BackdropStyle::transparent())),
            ..EngineConfig::default()
        },
        None,
    )
    .expect("GPU");
    let straight = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.8, 0.0, 0.0]];
    let far = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [6.0, 0.0, 0.0]];
    segment(&mut scene, owner, far, straight, 0);
    assert_eq!(coverage(&pixels(&mut engine, &scene)), 0);
    scene
        .set_trajectory_time(owner, 0.5)
        .expect("outside cutoff");
    assert_eq!(coverage(&pixels(&mut engine, &scene)), 0);
    scene
        .set_trajectory_time(owner, 1.0)
        .expect("inside cutoff");
    let accepted = pixels(&mut engine, &scene);
    assert!(coverage(&accepted) > 50);
    assert_eq!(accepted, pixels(&mut engine, &scene));
    let bent = [[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.8, 0.0, 0.0]];
    segment(&mut scene, owner, straight, bent, 2);
    assert_eq!(accepted, pixels(&mut engine, &scene));
    scene.set_trajectory_time(owner, 1.0).expect("bent pivot");
    assert_eq!(coverage(&pixels(&mut engine, &scene)), 0);
    scene.set_trajectory_time(owner, 0.0).expect("reverse");
    assert_eq!(accepted, pixels(&mut engine, &scene));
    let collapsed = [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [2.8, 0.0, 0.0]];
    segment(&mut scene, owner, straight, collapsed, 4);
    scene
        .set_trajectory_time(owner, 1.0)
        .expect("degenerate angle");
    assert_eq!(coverage(&pixels(&mut engine, &scene)), 0);
}

fn fixture_scene() -> Scene {
    let pdb = b"ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 20.00           N\nATOM      2  H   ALA A   1       1.000   0.000   0.000  1.00 20.00           H\nATOM      3  O   ALA A   2       2.800   0.000   0.000  1.00 20.00           O\nEND\n";
    let source = molframe::read_bytes(
        pdb.to_vec(),
        Some("oriented.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source")
    .0;
    Scene::from_structure(&source).expect("scene")
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn static_chemical_contacts_hide_when_either_molecule_moves_and_restore_the_original_pixels() {
    use molgfx_core::{InteractionAnchor, InteractionEdge, InteractionGeometry, InteractionKind};
    let mut scene = fixture_scene();
    let first = scene.structures().next().expect("first").0;
    let second = scene
        .add_source(scene.structure(first).expect("source").source.clone())
        .expect("second");
    let edge = InteractionEdge::new(
        first,
        InteractionAnchor::world(Vec3::ZERO).expect("start"),
        InteractionAnchor::world(Vec3::new(2.8, 0.0, 0.0)).expect("end"),
        InteractionKind::SaltBridge,
        InteractionGeometry::new(2.8, None).expect("distance"),
        "original static calculation",
    )
    .expect("edge")
    .with_static_coordinates([Some(first), Some(second)], &scene)
    .expect("scope");
    scene.add_interaction(edge).expect("edge");
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            profile: RenderProfile::bare()
                .with_effect(PresentationEffect::Backdrop(BackdropStyle::transparent())),
            ..EngineConfig::default()
        },
        None,
    )
    .expect("GPU");
    let original = pixels(&mut engine, &scene);
    assert!(coverage(&original) > 50);
    for owner in [second, first] {
        let before = scene.interaction_revision();
        segment(&mut scene, owner, [[0.0; 3]; 3], [[1.0; 3]; 3], 20);
        assert_eq!(coverage(&pixels(&mut engine, &scene)), 0);
        scene.set_trajectory_time(owner, 0.5).expect("move");
        assert_eq!(coverage(&pixels(&mut engine, &scene)), 0);
        scene.clear_trajectory(owner).expect("return to static");
        assert_eq!(original, pixels(&mut engine, &scene));
        assert_eq!(scene.interaction_revision(), before);
    }
    scene
        .structure_mut(second)
        .expect("placement")
        .model_to_world = molgfx_math::Mat4::from_translation(Vec3::X);
    assert_eq!(coverage(&pixels(&mut engine, &scene)), 0);
    scene
        .structure_mut(second)
        .expect("placement")
        .model_to_world = molgfx_math::Mat4::IDENTITY;
    assert_eq!(original, pixels(&mut engine, &scene));
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn scientific_candidates_and_snapshots_hide_when_their_original_coordinate_input_expires() {
    for interval_scope in [false, true] {
        let mut scene = fixture_scene();
        let owner = scene.structures().next().expect("owner").0;
        let straight = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.8, 0.0, 0.0]];
        segment(&mut scene, owner, straight, straight, 0);
        let placed = scene.structure(owner).expect("placed");
        let original = placed.trajectory().expect("original pair").clone();
        let pose = molgfx_core::MolecularPose::capture(placed);
        let domain = RowDomain::Atoms(owner);
        let batch = RelationBatch::new(
            Arc::from([Relation {
                start: SpatialAnchor::entity(RowEntityRef::new(domain, 0)).expect("donor"),
                end: SpatialAnchor::entity(RowEntityRef::new(domain, 2)).expect("acceptor"),
            }]),
            SourceRows::ordered(SourceNamespace(99), 1),
            RelationStyle {
                width_pixels: 4.0,
                ..RelationStyle::default()
            },
        )
        .expect("rows");
        let batch = if interval_scope {
            batch
                .with_molecular_interval(pose)
                .expect("candidate interval")
        } else {
            batch
                .with_molecular_sample(pose)
                .expect("classified sample")
        };
        let rows = batch.relations().clone();
        let handle = scene.add_relation_batch(batch).expect("bind");
        let mut engine = Engine::<WgpuDevice>::new(
            &EngineConfig {
                mode: RenderMode::Realtime,
                profile: RenderProfile::bare()
                    .with_effect(PresentationEffect::Backdrop(BackdropStyle::transparent())),
                ..EngineConfig::default()
            },
            None,
        )
        .expect("GPU");
        let accepted = pixels(&mut engine, &scene);
        assert!(coverage(&accepted) > 50);
        scene
            .set_trajectory_time(owner, 0.5)
            .expect("sample advances");
        let sampled = pixels(&mut engine, &scene);
        if interval_scope {
            assert_eq!(sampled, accepted);
        } else {
            assert_eq!(coverage(&sampled), 0);
        }
        segment(&mut scene, owner, straight, straight, 2);
        assert_eq!(coverage(&pixels(&mut engine, &scene)), 0);
        scene
            .set_trajectory_segment(owner, original)
            .expect("original owners return");
        assert_eq!(pixels(&mut engine, &scene), accepted);
        assert!(Arc::ptr_eq(
            &rows,
            scene
                .relation_batch(handle)
                .expect("retained rows")
                .relations()
        ));
    }
}
