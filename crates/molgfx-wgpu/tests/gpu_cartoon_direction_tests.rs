//! Source-anchored direction glyphs must deform and pick on the native GPU.
#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{
    AtomSelection, EntityKind, LogicalRow, Scene, TrajectoryFrame, TrajectorySegment,
};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PickEntity, PresentationEffect, RenderMode,
    RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn camera(eye: Vec3, up: Vec3) -> Camera {
    Camera {
        eye,
        target: Vec3::ZERO,
        up,
        projection: Projection::Orthographic {
            height: 4.0,
            aspect: 1.0,
            near: 0.1,
            far: 30.0,
        },
    }
}

fn assert_live_guide(
    source: &[u8],
    camera: Camera,
    frames: [[[f32; 3]; 3]; 2],
    moved_pick: [u32; 2],
) {
    let (source, _) = molframe::read_bytes(
        source.to_vec(),
        Some("direction.pdb"),
        &molframe::ReadOptions::new(),
    )
    .unwrap();
    let mut scene = Scene::from_structure(&source).unwrap();
    let owner = scene.structures().next().unwrap().0;
    let selection = scene.add_selection(AtomSelection::Sparse(vec![1]));
    scene
        .represent(
            selection,
            molgfx_core::RepresentationConfig::new(molgfx_core::RepresentationKind::Cartoon)
                .ribbon_width(3.0)
                .direction_wedges(true),
        )
        .unwrap();
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            profile: RenderProfile::bare()
                .with_effect(PresentationEffect::Backdrop(BackdropStyle::transparent())),
            ..EngineConfig::default()
        },
        None,
    )
    .unwrap();
    let extent = ImageConfig {
        width: 128,
        height: 128,
    };
    let before = engine.render_image(&scene, &camera, extent).unwrap();
    let pick = engine
        .pick(64, 64)
        .unwrap()
        .expect("the selected guide owns its wedge");
    assert!(
        matches!(pick.entity, PickEntity::Structure(identity) if identity.kind() == EntityKind::Atom && identity.row() == LogicalRow::new(1))
    );
    let start = TrajectoryFrame::new(1, 0.0, Arc::from(frames[0]), "direction:start").unwrap();
    let end = TrajectoryFrame::new(2, 1.0, Arc::from(frames[1]), "direction:end").unwrap();
    scene
        .set_trajectory_segment(owner, TrajectorySegment::new(start, end, 1.0).unwrap())
        .unwrap();
    let after = engine.render_image(&scene, &camera, extent).unwrap();
    assert_ne!(before.pixels, after.pixels);
    assert!(
        engine.pick(64, 64).unwrap().is_none(),
        "the glyph leaves its reference guide"
    );
    let pick = engine
        .pick(moved_pick[0], moved_pick[1])
        .unwrap()
        .expect("the moved guide remains pickable");
    assert!(
        matches!(pick.entity, PickEntity::Structure(identity) if identity.row() == LogicalRow::new(1))
    );
    let repeated = engine.render_image(&scene, &camera, extent).unwrap();
    assert_eq!(after.pixels, repeated.pixels);
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn an_isolated_residue_direction_follows_its_live_backbone_and_preserves_atom_picking() {
    assert_live_guide(
        b"ATOM      1  N   GLY A   1      -1.000   0.000   0.000  1.00 10.00           N  \nATOM      2  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \nATOM      3  C   GLY A   1       1.000   0.000   0.000  1.00 10.00           C  \nEND\n",
        camera(Vec3::new(0.0, 0.0, -10.0), Vec3::Y),
        [
            [[-1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            [[1.0, -1.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
        ],
        // Looking from negative Z mirrors the model's X axis on screen.
        [32, 64],
    );
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_single_selected_guide_deforms_from_its_parent_without_drawing_neighbor_residues() {
    assert_live_guide(
        b"ATOM      1  CA  GLY A   1      -2.000   0.000   0.000  1.00 10.00           C  \nATOM      2  CA  GLY A   2       0.000   0.000   0.000  1.00 10.00           C  \nATOM      3  CA  GLY A   3       2.000   0.000   0.000  1.00 10.00           C  \nEND\n",
        camera(Vec3::new(0.0, 10.0, 0.0), Vec3::Z),
        [
            [[-2.0, 0.0, 0.0], [0.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            [[-2.0, 0.0, 1.0], [0.0, 0.0, 1.0], [2.0, 0.0, 1.0]],
        ],
        [64, 32],
    );
}
