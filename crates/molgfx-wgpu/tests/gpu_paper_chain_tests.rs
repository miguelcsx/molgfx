//! Live topology-backed ring geometry and puckering colour on a native GPU.
#![cfg(not(target_arch = "wasm32"))]
use molgfx_core::{AtomSelection, RepresentationKind, Scene, TrajectoryFrame, TrajectorySegment};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn source() -> molframe::Structure {
    let pdb = b"HETATM    1  X1  UNK A   1       1.400   0.000   0.000  1.00 10.00           C\nHETATM    2  X2  UNK A   1       0.700   1.212   0.000  1.00 10.00           C\nHETATM    3  X3  UNK A   1      -0.700   1.212   0.000  1.00 10.00           C\nHETATM    4  X4  UNK A   1      -1.400   0.000   0.000  1.00 10.00           C\nHETATM    5  X5  UNK A   1      -0.700  -1.212   0.000  1.00 10.00           C\nHETATM    6  X6  UNK A   1       0.700  -1.212   0.000  1.00 10.00           C\nCONECT    1    2    6\nCONECT    2    3\nCONECT    3    4\nCONECT    4    5\nCONECT    5    6\nEND\n";
    molframe::read_bytes(
        pdb.to_vec(),
        Some("ring.pdb"),
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
            height: 8.0,
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
        .expect("ring renders")
        .pixels
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_ring_tracks_internal_puckering_and_height_without_rebuilding_its_topology() {
    let source = source();
    let mut scene = Scene::from_structure(&source).expect("scene");
    let owner = scene.structures().next().expect("placed source").0;
    let selection = scene.add_selection(AtomSelection::All);
    let representation = scene
        .represent(selection, RepresentationKind::PaperChain)
        .expect("ring form");
    let flat: Arc<[[f32; 3]]> = Arc::from(
        source
            .atoms()
            .into_iter()
            .map(|atom| atom.position().expect("coordinates"))
            .collect::<Vec<_>>(),
    );
    let mut chair = flat.to_vec();
    for (row, point) in chair.iter_mut().enumerate() {
        point[2] += if row % 2 == 0 { 0.5 } else { -0.5 };
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
    scene
        .set_trajectory_time(owner, 1.0)
        .expect("interpolation");
    let puckered = pixels(&mut engine, &scene);
    assert!(
        planar != puckered,
        "internal ring motion changes geometry and pucker colour"
    );
    scene.set_trajectory_time(owner, 0.0).expect("reverse");
    assert_eq!(
        planar,
        pixels(&mut engine, &scene),
        "reversing the motion restores the same geometry and palette"
    );
    scene
        .representation_mut(representation)
        .expect("height edit")
        .params
        .ribbon_width = 3.0;
    let taller = pixels(&mut engine, &scene);
    assert!(
        taller != planar,
        "the authored height controls the rendered bipyramid"
    );
}
