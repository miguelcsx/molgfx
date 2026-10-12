//! Schematic beta stencils and short helix fits track resident coordinates.
#![cfg(not(target_arch = "wasm32"))]
use molgfx_core::{AtomSelection, RepresentationKind, Scene, TrajectoryFrame, TrajectorySegment};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn engine() -> Engine<WgpuDevice> {
    Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            profile: RenderProfile::bare()
                .with_effect(PresentationEffect::Backdrop(BackdropStyle::transparent())),
            ..EngineConfig::default()
        },
        None,
    )
    .expect("GPU")
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_single_selected_helix_guide_tracks_the_complete_fit_and_keeps_selected_picking() {
    let source = source();
    let mut scene = Scene::from_structure(&source).expect("scene");
    let owner = scene.structures().next().expect("owner").0;
    scene
        .apply_secondary_structure(
            owner,
            &(0..3)
                .map(|row| {
                    (
                        molframe::ResidueIndex::new(row),
                        molgfx_core::SecondaryStructure::AlphaHelix,
                    )
                })
                .collect::<Vec<_>>(),
        )
        .expect("helix");
    let selected = scene.add_selection(AtomSelection::Sparse(vec![1]));
    scene
        .represent(selected, RepresentationKind::Rocket)
        .expect("single guide");
    let start: Arc<[[f32; 3]]> = Arc::from(source.coordinates());
    let mut moved = start.to_vec();
    moved[0][1] += 6.0;
    assert_eq!(start[1].map(f32::to_bits), moved[1].map(f32::to_bits));
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(
                TrajectoryFrame::new(0, 0.0, start, "source").expect("start"),
                TrajectoryFrame::new(1, 1.0, Arc::from(moved), "unselected motion").expect("end"),
                0.0,
            )
            .expect("segment"),
        )
        .expect("trajectory");
    let mut engine = engine();
    let before = pixels(&mut engine, &scene, 12.0);
    let coverage = before
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] > 128)
        .count();
    assert!(
        coverage > 100,
        "one selected guide has a closed piece of the full helix"
    );
    let index = before
        .as_chunks::<4>()
        .0
        .iter()
        .position(|pixel| pixel[3] > 128)
        .expect("selected piece");
    let pick = engine
        .pick(
            u32::try_from(index % 256).expect("x"),
            u32::try_from(index / 256).expect("y"),
        )
        .expect("readback")
        .expect("selected pick");
    assert!(
        matches!(pick.entity, molgfx_render::PickEntity::Structure(identity) if identity.row() == molgfx_core::LogicalRow::new(1))
    );
    scene
        .set_trajectory_time(owner, 1.0)
        .expect("unselected motion");
    assert!(
        before != pixels(&mut engine, &scene, 12.0),
        "the fit follows unselected guides without moving the selected source atom"
    );
    scene.set_trajectory_time(owner, 0.0).expect("reverse");
    assert!(
        before == pixels(&mut engine, &scene, 12.0),
        "reversal restores exact pixels"
    );
}

fn source() -> molframe::Structure {
    let pdb = "ATOM      1  CA  ALA A   1      -3.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  ALA A   2       0.000   2.000   0.000  1.00 10.00           C\nATOM      3  CA  ALA A   3       3.000   0.000   0.000  1.00 10.00           C\nEND\n";
    molframe::read_bytes(
        pdb.as_bytes().to_vec(),
        Some("trace.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("guides")
    .0
}

fn pixels(engine: &mut Engine<WgpuDevice>, scene: &Scene, height: f32) -> Vec<u8> {
    engine
        .render_image(
            scene,
            &Camera {
                eye: Vec3::new(0.0, 0.0, 20.0),
                target: Vec3::ZERO,
                up: Vec3::Y,
                projection: Projection::Orthographic {
                    height,
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
        .expect("polymer stroke render")
        .pixels
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn schematic_strands_and_three_guide_helices_follow_internal_motion_reversibly() {
    for style in [
        molgfx_core::SecondaryStructure::Strand,
        molgfx_core::SecondaryStructure::AlphaHelix,
    ] {
        let source = source();
        let mut scene = Scene::from_structure(&source).expect("scene");
        let owner = scene.structures().next().expect("owner").0;
        scene
            .apply_secondary_structure(
                owner,
                &(0..3)
                    .map(|index| (molframe::ResidueIndex::new(index), style))
                    .collect::<Vec<_>>(),
            )
            .expect("styles");
        let selection = scene.add_selection(AtomSelection::All);
        scene
            .represent(selection, RepresentationKind::Rocket)
            .expect("schematic");
        let start: Arc<[[f32; 3]]> = Arc::from(source.coordinates());
        let mut moved = start.to_vec();
        moved[1][0] += 1.5;
        moved[1][1] += 1.0;
        scene
            .set_trajectory_segment(
                owner,
                TrajectorySegment::new(
                    TrajectoryFrame::new(0, 0.0, start, "start").expect("start"),
                    TrajectoryFrame::new(1, 1.0, Arc::from(moved), "moved").expect("end"),
                    0.0,
                )
                .expect("segment"),
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
        let before = pixels(&mut engine, &scene, 12.0);
        assert!(
            before
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[3] > 128)
                .count()
                > 200,
            "{style:?} must render"
        );
        scene.set_trajectory_time(owner, 1.0).expect("move");
        assert_ne!(before, pixels(&mut engine, &scene, 12.0), "{style:?}");
        scene.set_trajectory_time(owner, 0.0).expect("reverse");
        assert_eq!(before, pixels(&mut engine, &scene, 12.0), "{style:?}");
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn selected_coils_follow_hidden_secondary_boundaries_and_reverse_to_identical_pixels() {
    let pdb = "ATOM      1  CA  GLY A   1      -6.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  GLY A   2      -3.000   0.000   0.000  1.00 10.00           C\nATOM      3  CA  GLY A   3       0.000   2.000   0.000  1.00 10.00           C\nATOM      4  CA  GLY A   4       3.000   0.000   0.000  1.00 10.00           C\nATOM      5  CA  GLY A   5       6.000   0.000   0.000  1.00 10.00           C\nEND\n";
    let structure = molframe::read_bytes(
        pdb.as_bytes().to_vec(),
        Some("joins.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source")
    .0;
    for state in [
        molgfx_core::SecondaryStructure::AlphaHelix,
        molgfx_core::SecondaryStructure::Strand,
    ] {
        let mut scene = Scene::from_structure(&structure).expect("scene");
        let owner = scene.structures().next().expect("source").0;
        let mut styles: Vec<_> = (0..5)
            .map(|row| (molframe::ResidueIndex::new(row), state))
            .collect();
        styles[0].1 = molgfx_core::SecondaryStructure::Coil;
        styles[4].1 = molgfx_core::SecondaryStructure::Coil;
        scene
            .apply_secondary_structure(owner, &styles)
            .expect("styles");
        let selection = scene.add_selection(AtomSelection::Sparse(vec![0]));
        scene
            .represent(selection, RepresentationKind::Rocket)
            .expect("coil");
        let start: Arc<[[f32; 3]]> = Arc::from(structure.coordinates());
        let mut moved = start.to_vec();
        moved[2][1] += 3.0;
        assert_eq!(start[0].map(f32::to_bits), moved[0].map(f32::to_bits));
        scene
            .set_trajectory_segment(
                owner,
                TrajectorySegment::new(
                    TrajectoryFrame::new(0, 0.0, start, "initial").expect("start"),
                    TrajectoryFrame::new(1, 1.0, Arc::from(moved), "hidden guide moved")
                        .expect("end"),
                    0.0,
                )
                .expect("segment"),
            )
            .expect("trajectory");
        let mut renderer = engine();
        let before = pixels(&mut renderer, &scene, 16.0);
        let index = before
            .as_chunks::<4>()
            .0
            .iter()
            .position(|pixel| pixel[3] > 128)
            .expect("visible connection");
        let pick = renderer
            .pick(
                u32::try_from(index % 256).expect("x"),
                u32::try_from(index / 256).expect("y"),
            )
            .expect("readback")
            .expect("hit");
        assert!(
            matches!(pick.entity, molgfx_render::PickEntity::Structure(identity) if identity.row() == molgfx_core::LogicalRow::new(0))
        );
        scene
            .set_trajectory_time(owner, 1.0)
            .expect("move hidden guide");
        assert!(
            before != pixels(&mut renderer, &scene, 16.0),
            "the coil follows the {state:?} boundary although its selected CA never moves"
        );
        scene.set_trajectory_time(owner, 0.0).expect("reverse");
        assert!(
            before == pixels(&mut renderer, &scene, 16.0),
            "reversal restores exact {state:?} pixels"
        );
    }
}
