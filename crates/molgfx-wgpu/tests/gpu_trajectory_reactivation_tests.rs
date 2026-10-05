//! Trajectory reactivation must not blur from a previous activation.
#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{AtomSelection, RepresentationKind, Scene, TrajectoryFrame, TrajectorySegment};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    AdaptiveQualityConfig, BackdropStyle, Engine, EngineConfig, ImageConfig, MotionBlur,
    PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn engine() -> Engine<WgpuDevice> {
    Engine::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            profile: RenderProfile::bare()
                .with_effect(PresentationEffect::Backdrop(BackdropStyle::transparent()))
                .with_effect(PresentationEffect::MotionBlur(MotionBlur::restrained())),
            ..EngineConfig::default()
        },
        None,
    )
    .expect("native GPU opens")
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn removing_a_trajectory_restores_the_exact_converged_source_image() {
    let (source, _) = molframe::read_bytes(
        b"ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \n\
ATOM      2  CA  ALA A   2       4.000   0.000   0.000  1.00 10.00           C  \nEND\n"
            .to_vec(),
        Some("trajectory.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source parses");
    let mut scene = Scene::from_structure(&source).expect("source binds");
    let owner = scene.structures().next().expect("structure exists").0;
    let selection = scene.add_selection(AtomSelection::All);
    scene
        .represent(selection, RepresentationKind::Spacefill)
        .expect("atoms attach");
    let camera = Camera {
        eye: Vec3::new(2.0, 0.0, 15.0),
        target: Vec3::new(2.0, 0.0, 0.0),
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 10.0,
            aspect: 1.0,
            near: 0.1,
            far: 30.0,
        },
    };
    let config = EngineConfig {
        adaptive: AdaptiveQualityConfig::highest_fixed(60),
        profile: RenderProfile::bare()
            .with_effect(PresentationEffect::MotionBlur(MotionBlur::restrained())),
        ..EngineConfig::default()
    };
    let mut renderer = Engine::<WgpuDevice>::new(&config, None).expect("native GPU opens");
    let extent = ImageConfig {
        width: 128,
        height: 128,
    };
    let original = renderer
        .render_image(&scene, &camera, extent)
        .expect("source image renders");
    let start = TrajectoryFrame::new(
        0,
        0.0,
        Arc::from([[0.0, 0.0, 0.0], [4.0, 0.0, 0.0]]),
        "start",
    )
    .expect("start validates");
    let end = TrajectoryFrame::new(
        1,
        1.0,
        Arc::from([[0.0, 3.0, 0.0], [4.0, -3.0, 0.0]]),
        "end",
    )
    .expect("end validates");
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(start, end, 0.25).expect("sample validates"),
        )
        .expect("trajectory attaches");
    let moved = renderer
        .render_image(&scene, &camera, extent)
        .expect("trajectory image renders");
    assert_ne!(original.pixels, moved.pixels);
    scene.clear_trajectory(owner).expect("trajectory detaches");
    let restored = renderer
        .render_image(&scene, &camera, extent)
        .expect("restored image renders");
    assert_eq!(original.pixels, restored.pixels);
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn reactivated_trajectory_motion_blur_matches_a_fresh_activation_at_the_same_sample() {
    let (source, _) = molframe::read_bytes(
        b"ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n"
            .to_vec(),
        Some("trajectory.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source parses");
    let mut scene = Scene::from_structure(&source).expect("source binds");
    let owner = scene.structures().next().expect("structure exists").0;
    let selection = scene.add_selection(AtomSelection::All);
    scene
        .represent(selection, RepresentationKind::Spacefill)
        .expect("atoms attach");
    let start = TrajectoryFrame::new(0, 0.0, Arc::from([[0.0, 0.0, 0.0]]), "start")
        .expect("start frame validates");
    let end = TrajectoryFrame::new(1, 1.0, Arc::from([[4.0, 0.0, 0.0]]), "end")
        .expect("end frame validates");
    let mut segment = TrajectorySegment::new(start, end, 0.25).expect("segment validates");
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 10.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 10.0,
            aspect: 1.0,
            near: 0.1,
            far: 30.0,
        },
    };
    let extent = ImageConfig {
        width: 128,
        height: 128,
    };
    let mut reused = engine();
    scene
        .set_trajectory_segment(owner, segment.clone())
        .expect("trajectory attaches");
    reused
        .render_image(&scene, &camera, extent)
        .expect("initial sample renders");
    scene.clear_trajectory(owner).expect("trajectory detaches");
    reused
        .render_image(&scene, &camera, extent)
        .expect("source frame renders");
    segment
        .set_sample_time(0.75)
        .expect("later sample validates");
    scene
        .set_trajectory_segment(owner, segment)
        .expect("trajectory reattaches");
    let reactivated = reused
        .render_image(&scene, &camera, extent)
        .expect("reactivation renders");
    let fresh = engine()
        .render_image(&scene, &camera, extent)
        .expect("fresh activation renders");
    assert_eq!(
        reactivated.pixels, fresh.pixels,
        "reactivation must not retain stale trajectory motion"
    );
}
