//! Distance-defined triangles on the native GPU and borrowed trajectory seam.
#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{AtomSelection, RepresentationKind, Scene, TrajectoryFrame, TrajectorySegment};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

#[test]
#[ignore = "requires a native GPU adapter"]
fn polyhedra_faces_appear_and_disappear_at_the_exact_interpolated_distance_cutoff() {
    let pdb = b"HETATM    1  C1  UNK A   1       0.000   0.000   0.000  1.00 10.00           C\nHETATM    2  C2  UNK A   1       4.000   0.000   0.000  1.00 10.00           C\nHETATM    3  C3  UNK A   1       0.000   4.000   0.000  1.00 10.00           C\nHETATM    4  C4  UNK A   1      -4.000   0.000   0.000  1.00 10.00           C\nEND\n";
    let (source, _) = molframe::read_bytes(
        pdb.to_vec(),
        Some("triangle.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source parses");
    let mut scene = Scene::from_structure(&source).expect("source binds");
    let owner = scene.structures().next().expect("source placed").0;
    let selection = scene.add_selection(AtomSelection::Sparse(vec![0]));
    scene
        .represent(selection, RepresentationKind::Polyhedra)
        .expect("form binds");
    let start = TrajectoryFrame::new(
        0,
        0.0,
        Arc::from([
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [0.0, 4.0, 0.0],
            [-4.0, 0.0, 0.0],
        ]),
        "start",
    )
    .expect("start");
    let end = TrajectoryFrame::new(
        1,
        1.0,
        Arc::from([
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
        ]),
        "end",
    )
    .expect("end");
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(start, end, 0.0).expect("segment"),
        )
        .expect("trajectory binds");
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            profile: RenderProfile::bare()
                .with_effect(PresentationEffect::Backdrop(BackdropStyle::transparent())),
            ..EngineConfig::default()
        },
        None,
    )
    .expect("GPU opens");
    let camera = camera();
    let mut coverage = |scene: &Scene| {
        engine
            .render_image(
                scene,
                &camera,
                ImageConfig {
                    width: 128,
                    height: 128,
                },
            )
            .expect("frame renders")
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] != 0)
            .count()
    };
    assert_eq!(
        coverage(&scene),
        0,
        "all pairs are beyond the cutoff initially"
    );
    scene
        .set_trajectory_time(owner, 0.25)
        .expect("time advances");
    assert_eq!(
        coverage(&scene),
        0,
        "the early interpolated neighbors remain beyond the central cutoff"
    );
    scene
        .set_trajectory_time(owner, 1.0)
        .expect("time advances");
    assert!(
        coverage(&scene) > 100,
        "the same candidate triangle becomes visible without a topology upload"
    );
    scene
        .set_trajectory_time(owner, 0.0)
        .expect("time reverses");
    assert_eq!(
        coverage(&scene),
        0,
        "the face disappears again in the same scene"
    );
}

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.5, 0.5, 20.0),
        target: Vec3::new(0.5, 0.5, 0.0),
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 4.0,
            aspect: 1.0,
            near: 0.1,
            far: 40.0,
        },
    }
}
