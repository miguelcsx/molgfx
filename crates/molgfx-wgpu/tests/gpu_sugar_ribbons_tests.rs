//! Nucleotide ribbon faces gather their full sugar ring on a native GPU.
#![cfg(not(target_arch = "wasm32"))]
use molgfx_core::{AtomSelection, RepresentationKind, Scene, TrajectoryFrame, TrajectorySegment};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn source() -> molframe::Structure {
    let cif = "data_sugar\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\nATOM 1 O \"O4'\" DA A 1 1 0 1.5 0\nATOM 2 C \"C1'\" DA A 1 1 1.4 0.5 0\nATOM 3 C \"C2'\" DA A 1 1 0.9 -1.2 0\nATOM 4 C \"C3'\" DA A 1 1 -0.9 -1.2 0\nATOM 5 C \"C4'\" DA A 1 1 -1.4 0.5 0\nATOM 6 N N1 DA A 1 1 3 0 0\nATOM 7 C C2 DA A 1 1 4 0 0\nATOM 8 N N3 DA A 1 1 4.5 1 0\nATOM 9 C C4 DA A 1 1 4 2 0\nATOM 10 C C5 DA A 1 1 3 2 0\nATOM 11 C C6 DA A 1 1 2.5 1 0\n";
    molframe::read_bytes(
        cif.as_bytes().to_vec(),
        Some("sugar.cif"),
        &molframe::ReadOptions::new(),
    )
    .expect("sugar")
    .0
}

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
                width: 256,
                height: 256,
            },
        )
        .expect("sugar face render")
        .pixels
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn sugar_faces_follow_every_ring_atom_without_a_polymer_trace() {
    for kind in [RepresentationKind::Ribbons, RepresentationKind::NewRibbons] {
        verify_faces(kind);
    }
}

fn verify_faces(kind: RepresentationKind) {
    let source = source();
    let mut scene = Scene::from_structure(&source).expect("scene");
    let owner = scene.structures().next().expect("owner").0;
    let selection = scene.add_selection(AtomSelection::All);
    scene.represent(selection, kind).expect("ribbons");
    let flat: Arc<[[f32; 3]]> = Arc::from(source.coordinates());
    let mut rotated = flat.to_vec();
    let pivot = flat[0];
    for point in &mut rotated {
        point[2] += point[1] - pivot[1];
        point[1] = pivot[1];
    }
    assert_eq!(rotated[0].map(f32::to_bits), pivot.map(f32::to_bits));
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(
                TrajectoryFrame::new(0, 0.0, flat, "flat").expect("start"),
                TrajectoryFrame::new(1, 1.0, Arc::from(rotated), "rotated").expect("end"),
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
    let start = pixels(&mut engine, &scene);
    assert!(
        start
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] > 0)
            .count()
            > 100
    );
    assert_eq!(start, pixels(&mut engine, &scene));
    scene.set_trajectory_time(owner, 1.0).expect("rotate");
    let edge_on = pixels(&mut engine, &scene);
    let coverage = |image: &[u8]| {
        image
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] > 0)
            .count()
    };
    assert!(
        coverage(&edge_on) * 4 < coverage(&start),
        "live sugar plane must turn edge on"
    );
    scene.set_trajectory_time(owner, 0.0).expect("reverse");
    assert_eq!(start, pixels(&mut engine, &scene));
}
