//! Live double-edge spline geometry on a native GPU.
#![cfg(not(target_arch = "wasm32"))]
use molgfx_core::{AtomSelection, RepresentationKind, Scene, TrajectoryFrame, TrajectorySegment};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn source() -> molframe::Structure {
    let pdb = b"ATOM      1  CA  ALA A   1      -4.000   0.000   0.000  1.00 10.00           C\nATOM      2  O   ALA A   1      -4.000   1.500   0.000  1.00 10.00           O\nATOM      3  CA  ALA A   2       0.000   0.000   0.000  1.00 10.00           C\nATOM      4  O   ALA A   2       0.000   1.500   0.000  1.00 10.00           O\nATOM      5  CA  ALA A   3       4.000   0.000   0.000  1.00 10.00           C\nATOM      6  O   ALA A   3       4.000   1.500   0.000  1.00 10.00           O\nEND\n";
    molframe::read_bytes(
        pdb.to_vec(),
        Some("ribbons.pdb"),
        &molframe::ReadOptions::new(),
    )
    .unwrap()
    .0
}

fn pixels(engine: &mut Engine<WgpuDevice>, scene: &Scene) -> Vec<u8> {
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 20.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 12.0,
            aspect: 1.0,
            near: 0.1,
            far: 40.0,
        },
    };
    engine
        .render_image(
            scene,
            &camera,
            ImageConfig {
                width: 256,
                height: 256,
            },
        )
        .expect("ribbons render")
        .pixels
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_double_edge_ribbon_follows_oxygen_only_motion_and_border_radius() {
    let source = source();
    let mut scene = Scene::from_structure(&source).expect("scene");
    let owner = scene.structures().next().expect("placed source").0;
    let selection = scene.add_selection(AtomSelection::All);
    let representation = scene
        .represent(selection, RepresentationKind::Ribbons)
        .expect("ribbon form");
    let flat: Arc<[[f32; 3]]> = Arc::from(
        source
            .atoms()
            .into_iter()
            .map(|atom| atom.position().expect("coordinates"))
            .collect::<Vec<_>>(),
    );
    let mut chair = flat.to_vec();
    for (row, point) in chair.iter_mut().enumerate() {
        if row % 2 == 1 {
            point[2] = point[1];
            point[1] = 0.0;
        }
    }
    let segment = TrajectorySegment::new(
        TrajectoryFrame::new(0, 0.0, flat, "flat").expect("start"),
        TrajectoryFrame::new(1, 1.0, Arc::from(chair), "chair").expect("end"),
        0.0,
    )
    .expect("segment");
    scene
        .set_trajectory_segment(owner, segment)
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
    let planar = pixels(&mut engine, &scene);
    assert!(
        planar
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] != 0)
            .count()
            > 100
    );
    let repeat = pixels(&mut engine, &scene);
    assert!(
        planar == repeat,
        "an unchanged ribbon must render the same pixels"
    );
    scene
        .set_trajectory_time(owner, 1.0)
        .expect("interpolation");
    let puckered = pixels(&mut engine, &scene);
    assert!(
        planar != puckered,
        "oxygen motion rotates both borders while alpha carbons remain fixed"
    );
    scene.set_trajectory_time(owner, 0.0).expect("reverse");
    assert!(
        planar == pixels(&mut engine, &scene),
        "reversing the motion restores both border splines"
    );
    scene
        .representation_mut(representation)
        .expect("height edit")
        .params
        .bond_radius = 0.8;
    let taller = pixels(&mut engine, &scene);
    assert!(
        taller != planar,
        "the authored border radius changes the circular border tubes"
    );
}
