//! Representation differences and source-derived geometry on a native GPU.
#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{AtomSelection, RepresentationKind, Scene, TrajectoryFrame, TrajectorySegment};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn engine() -> Engine<WgpuDevice> {
    Engine::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            profile: RenderProfile::bare()
                .with_effect(PresentationEffect::Backdrop(BackdropStyle::transparent())),
            ..EngineConfig::default()
        },
        None,
    )
    .expect("native GPU opens")
}

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, 20.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 12.0,
            aspect: 1.0,
            near: 0.1,
            far: 40.0,
        },
    }
}

fn scene(source: &[u8], kind: RepresentationKind) -> Scene {
    let (source, _) = molframe::read_bytes(
        source.to_vec(),
        Some("geometry.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("fixture parses");
    let mut scene = Scene::from_structure(&source).expect("source binds");
    let selection = scene.add_selection(AtomSelection::All);
    scene.represent(selection, kind).expect("form attaches");
    scene
}

fn image(engine: &mut Engine<WgpuDevice>, scene: &Scene) -> Vec<u8> {
    engine
        .render_image(
            scene,
            &camera(),
            ImageConfig {
                width: 128,
                height: 128,
            },
        )
        .expect("form renders")
        .pixels
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn trace_and_tube_have_distinct_corner_geometry_on_the_gpu() {
    let source = b"ATOM      1  CA  GLY A   1      -3.000  -2.000   0.000  1.00 10.00           C  \nATOM      2  CA  GLY A   2       0.000  -2.000   0.000  1.00 10.00           C  \nATOM      3  CA  GLY A   3       0.000   2.000   0.000  1.00 10.00           C  \nEND\n";
    let mut engine = engine();
    let trace = image(&mut engine, &scene(source, RepresentationKind::Trace));
    let tube = image(&mut engine, &scene(source, RepresentationKind::Tube));
    assert_ne!(trace, tube);
    assert!(trace.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn dots_cover_an_atom_surface_and_lines_keep_bonded_endpoints_visible() {
    let source = b"ATOM      1  C   GLY A   1      -0.750   0.000   0.000  1.00 10.00           C  \nATOM      2  O   GLY A   1       0.750   0.000   0.000  1.00 10.00           O  \nCONECT    1    2\nEND\n";
    let mut engine = engine();
    let points = image(&mut engine, &scene(source, RepresentationKind::Points));
    let dots = image(&mut engine, &scene(source, RepresentationKind::Dots));
    let covered = |pixels: &[u8]| {
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] != 0)
            .count()
    };
    assert!(covered(&dots) > covered(&points) * 4);
    let lines = image(&mut engine, &scene(source, RepresentationKind::Lines));
    assert_ne!(lines, points);
    assert!(engine.pick(64, 64).expect("bond pick succeeds").is_some());
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_residue_bead_changes_when_a_member_moves_without_moving_its_pick_anchor() {
    let source = b"ATOM      1  N   GLY A   1      -1.000   0.000   0.000  1.00 10.00           N  \nATOM      2  CA  GLY A   1       1.000   0.000   0.000  1.00 10.00           C  \nEND\n";
    let mut scene = scene(source, RepresentationKind::Beads);
    let owner = scene.structures().next().expect("owner exists").0;
    let mut engine = engine();
    let before = image(&mut engine, &scene);
    let start = TrajectoryFrame::new(
        0,
        0.0,
        Arc::from([[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]]),
        "bead-start",
    )
    .expect("start validates");
    let end = TrajectoryFrame::new(
        1,
        1.0,
        Arc::from([[-1.0, 0.0, 0.0], [3.0, 0.0, 0.0]]),
        "bead-end",
    )
    .expect("end validates");
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(start, end, 1.0).expect("segment validates"),
        )
        .expect("trajectory attaches");
    let after = image(&mut engine, &scene);
    assert_ne!(before, after);
    scene.clear_trajectory(owner).expect("source restores");
    assert_eq!(before, image(&mut engine, &scene));
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn selection_and_hover_change_color_without_changing_the_picked_entity() {
    let source =
        b"ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n";
    let mut scene = scene(source, RepresentationKind::Spacefill);
    let owner = scene.structures().next().expect("owner exists").0;
    let mut engine = engine();
    let before = image(&mut engine, &scene);
    let picked = engine
        .pick(64, 64)
        .expect("atom picks")
        .map(|pick| pick.entity);
    for channel in [
        molgfx_core::InteractionState::SELECTED,
        molgfx_core::InteractionState::HOVERED,
    ] {
        scene
            .update_interaction_channel(owner, channel.bits(), Some(&AtomSelection::All))
            .expect("channel updates");
        let marked = image(&mut engine, &scene);
        assert_ne!(before, marked);
        assert_eq!(
            picked,
            engine
                .pick(64, 64)
                .expect("marked atom picks")
                .map(|pick| pick.entity)
        );
        scene
            .update_interaction_channel(owner, channel.bits(), None)
            .expect("channel clears");
        assert_eq!(before, image(&mut engine, &scene));
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_rocket_refits_when_an_internal_guide_moves_with_both_endpoints_fixed() {
    let source = b"ATOM      1  CA  ALA A   1      -1.000  -4.000   0.000  1.00 10.00           C  \nATOM      2  CA  ALA A   2       1.000  -2.000   0.000  1.00 10.00           C  \nATOM      3  CA  ALA A   3      -1.000   0.000   0.000  1.00 10.00           C  \nATOM      4  CA  ALA A   4       1.000   2.000   0.000  1.00 10.00           C  \nATOM      5  CA  ALA A   5      -1.000   4.000   0.000  1.00 10.00           C  \nEND\n";
    let mut scene = scene(source, RepresentationKind::Rocket);
    let owner = scene.structures().next().expect("owner exists").0;
    scene
        .structure_mut(owner)
        .expect("owner resolves")
        .secondary_structure
        .values_mut()
        .fill(molgfx_core::SecondaryStructure::AlphaHelix);
    let mut engine = engine();
    let before = image(&mut engine, &scene);
    let original = [
        [-1.0, -4.0, 0.0],
        [1.0, -2.0, 0.0],
        [-1.0, 0.0, 0.0],
        [1.0, 2.0, 0.0],
        [-1.0, 4.0, 0.0],
    ];
    let mut moved = original;
    moved[2][0] = 3.0;
    let start =
        TrajectoryFrame::new(0, 0.0, Arc::from(original), "rocket-start").expect("start validates");
    let end = TrajectoryFrame::new(1, 1.0, Arc::from(moved), "rocket-end").expect("end validates");
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(start, end, 1.0).expect("segment validates"),
        )
        .expect("segment attaches");
    assert_ne!(before, image(&mut engine, &scene));
    scene.clear_trajectory(owner).expect("source restores");
    assert_eq!(before, image(&mut engine, &scene));
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn stereo_eye_renders_have_disparity_and_compose_the_exact_eye_pixels() {
    let source =
        b"ATOM      1  CA  GLY A   1       0.000   0.000   5.000  1.00 10.00           C  \nEND\n";
    let scene = scene(source, RepresentationKind::Spacefill);
    let mut engine = engine();
    let mut camera = camera();
    camera.projection = Projection::Perspective {
        fov_y: 0.8,
        aspect: 1.0,
        near: 0.1,
        far: 100.0,
    };
    let eyes = engine
        .render_stereo_images(
            &scene,
            &camera,
            ImageConfig {
                width: 128,
                height: 128,
            },
            1.2,
            20.0,
        )
        .expect("stereo renders");
    assert_ne!(eyes.left.pixels, eyes.right.pixels);
    let pair = eyes
        .compose(molgfx_render::StereoLayout::SideBySide)
        .expect("pair composes");
    assert_eq!((pair.width, pair.height), (256, 128));
    for row in 0..128usize {
        assert_eq!(
            &pair.pixels[row * 1024..row * 1024 + 512],
            &eyes.left.pixels[row * 512..(row + 1) * 512]
        );
        assert_eq!(
            &pair.pixels[row * 1024 + 512..(row + 1) * 1024],
            &eyes.right.pixels[row * 512..(row + 1) * 512]
        );
    }
    assert!(
        eyes.left
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] > 0)
    );
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn base_polygons_follow_internal_ring_motion_with_sugar_and_glycosidic_anchors_fixed() {
    let source = b"ATOM      1  C1'  DC A   1      -2.400   0.000   0.000  1.00 10.00           C  \nATOM      2  N1   DC A   1      -1.200   0.000   0.000  1.00 10.00           N  \nATOM      3  C2   DC A   1      -0.600   1.039   0.000  1.00 10.00           C  \nATOM      4  N3   DC A   1       0.600   1.039   0.000  1.00 10.00           N  \nATOM      5  C4   DC A   1       1.200   0.000   0.000  1.00 10.00           C  \nATOM      6  C5   DC A   1       0.600  -1.039   0.000  1.00 10.00           C  \nATOM      7  C6   DC A   1      -0.600  -1.039   0.000  1.00 10.00           C  \nEND\n";
    let mut scene = scene(source, RepresentationKind::Bases);
    let owner = scene.structures().next().expect("structure").0;
    let coordinates = [
        [-2.4, 0.0, 0.0],
        [-1.2, 0.0, 0.0],
        [-0.6, 1.039, 0.0],
        [0.6, 1.039, 0.0],
        [1.2, 0.0, 0.0],
        [0.6, -1.039, 0.0],
        [-0.6, -1.039, 0.0],
    ];
    let mut moved = coordinates;
    moved[4][0] = 3.0;
    let start = TrajectoryFrame::new(0, 0.0, Arc::from(coordinates), "ring-start").expect("start");
    let end = TrajectoryFrame::new(1, 1.0, Arc::from(moved), "ring-end").expect("end");
    let mut engine = engine();
    let before = image(&mut engine, &scene);
    assert!(before.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(start, end, 1.0).expect("segment"),
        )
        .expect("motion");
    let after = image(&mut engine, &scene);
    assert_ne!(before, after);
    scene.clear_trajectory(owner).expect("clear");
    assert_eq!(before, image(&mut engine, &scene));
}
