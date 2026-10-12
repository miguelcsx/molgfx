//! Classified interaction strokes retain live atom anchors and row picking.
#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{
    EntityKind, LogicalRow, Relation, RelationBatch, RelationPattern, RelationStyle, RowDomain,
    RowEntityRef, Scene, SourceNamespace, SourceRows, SpatialAnchor, TrajectoryFrame,
    TrajectorySegment,
};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PickEntity, PresentationEffect, RenderMode,
    RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn pixels(engine: &mut Engine<WgpuDevice>, scene: &Scene) -> Vec<u8> {
    engine
        .render_image(
            scene,
            &Camera {
                eye: Vec3::new(0.0, 0.0, 20.0),
                target: Vec3::ZERO,
                up: Vec3::Y,
                projection: Projection::Orthographic {
                    height: 12.0,
                    aspect: 1.0,
                    near: 0.1,
                    far: 40.0,
                },
            },
            ImageConfig {
                width: 128,
                height: 128,
            },
        )
        .expect("interaction render")
        .pixels
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn classified_strokes_follow_live_atom_coordinates_and_keep_relation_picking() {
    let pdb = b"ATOM      1  N   ALA A   1      -4.000   0.000   0.000  1.00 20.00           N\nATOM      2  H   ALA A   1      -3.000   0.000   0.000  1.00 20.00           H\nATOM      3  O   ALA A   2       4.000   0.000   0.000  1.00 20.00           O\nEND\n";
    let (source, _) = molframe::read_bytes(
        pdb.to_vec(),
        Some("classified.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source");
    let mut scene = Scene::from_structure(&source).expect("scene");
    let owner = scene.structures().next().expect("source owner").0;
    let anchor = |row| {
        SpatialAnchor::entity(RowEntityRef::new(RowDomain::Atoms(owner), row)).expect("atom anchor")
    };
    scene
        .add_relation_batch(
            RelationBatch::new(
                Arc::from([Relation {
                    start: anchor(0),
                    end: anchor(2),
                }]),
                SourceRows::keyed(SourceNamespace(98), Arc::from([701])).expect("analysis row"),
                RelationStyle {
                    width_pixels: 4.0,
                    pattern: RelationPattern::Dashed,
                    ..RelationStyle::default()
                },
            )
            .expect("classified strokes"),
        )
        .expect("stroke insertion");
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(
                TrajectoryFrame::new(
                    0,
                    0.0,
                    Arc::from([[-4.0, 0.0, 0.0], [-3.0, 0.0, 0.0], [4.0, 0.0, 0.0]]),
                    "start",
                )
                .expect("start"),
                TrajectoryFrame::new(
                    1,
                    1.0,
                    Arc::from([[-4.0, 3.0, 0.0], [-3.0, 3.0, 0.0], [4.0, 3.0, 0.0]]),
                    "end",
                )
                .expect("end"),
                0.0,
            )
            .expect("motion"),
        )
        .expect("trajectory");
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
    let start = pixels(&mut engine, &scene);
    let (index, _) = start
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .find(|(_, pixel)| pixel[3] == 255)
        .expect("opaque dash coverage");
    let x = u32::try_from(index % 128).expect("pixel x");
    let y = u32::try_from(index / 128).expect("pixel y");
    let picked = engine.pick(x, y).expect("stroke pick").expect("relation");
    assert!(matches!(picked.entity, PickEntity::Structure(identity)
        if identity.kind() == EntityKind::Relation && identity.row() == LogicalRow::new(701)));
    assert_eq!(start, pixels(&mut engine, &scene));
    scene.set_trajectory_time(owner, 1.0).expect("move");
    assert_ne!(start, pixels(&mut engine, &scene));
    assert!(engine.pick(x, y).expect("old stroke position").is_none());
    scene.set_trajectory_time(owner, 0.0).expect("reverse");
    assert_eq!(start, pixels(&mut engine, &scene));
}
