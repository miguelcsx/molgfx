//! Ring-oriented glycosidic ribbons gather full rings on a native GPU.
#![cfg(not(target_arch = "wasm32"))]
use molgfx_core::{AtomSelection, RepresentationKind, Scene, TrajectoryFrame, TrajectorySegment};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::fmt::Write as _;
use std::sync::Arc;

fn source() -> molframe::Structure {
    let mut pdb = String::new();
    let names = ["C1", "C2", "C3", "C4", "C5", "O5", "O4"];
    for residue in 0..2u8 {
        let center = -3.0 + f32::from(residue) * 6.0;
        for (corner, name) in names.iter().enumerate() {
            let angle =
                f32::from(u8::try_from(corner).expect("corner")) * std::f32::consts::TAU / 6.0;
            let (x, y) = if corner == 6 {
                (center - 2.8, 0.0)
            } else {
                (center + 1.4 * angle.cos(), 1.4 * angle.sin())
            };
            let element = if name.starts_with('O') { "O" } else { "C" };
            let serial = usize::from(residue) * 7 + corner + 1;
            writeln!(pdb, "HETATM{serial:>5} {name:>4} NAG A{:>4}    {x:>8.3}{y:>8.3}{:>8.3}  1.00 10.00          {element:>2}", residue + 1, 0.0).expect("atom");
        }
    }
    for residue in 0..2u8 {
        let base = u32::from(residue) * 7 + 1;
        for corner in 0..6u32 {
            writeln!(
                pdb,
                "CONECT{:>5}{:>5}",
                base + corner,
                base + (corner + 1) % 6
            )
            .expect("ring bond");
        }
        writeln!(pdb, "CONECT{:>5}{:>5}", base + 3, base + 6).expect("acceptor bond");
    }
    pdb.push_str("CONECT    1   14\nEND\n");
    molframe::read_bytes(
        pdb.into_bytes(),
        Some("twister.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("sugars")
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
        .expect("twister render")
        .pixels
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_twister_follows_ring_planes_when_its_representative_oxygen_stays_fixed() {
    let source = source();
    let mut scene = Scene::from_structure(&source).expect("scene");
    let owner = scene.structures().next().expect("owner").0;
    let selection = scene.add_selection(AtomSelection::All);
    scene
        .represent(selection, RepresentationKind::Twister)
        .expect("glycan");
    let flat: Arc<[[f32; 3]]> = Arc::from(source.coordinates());
    let mut rotated = flat.to_vec();
    for residue in 0..2 {
        let pivot = flat[residue * 7 + 5];
        for point in &mut rotated[residue * 7..residue * 7 + 7] {
            point[2] += point[1] - pivot[1];
            point[1] = pivot[1];
        }
        assert_eq!(
            rotated[residue * 7 + 5].map(f32::to_bits),
            pivot.map(f32::to_bits)
        );
    }
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
    assert_ne!(start, pixels(&mut engine, &scene));
    scene.set_trajectory_time(owner, 0.0).expect("reverse");
    assert_eq!(start, pixels(&mut engine, &scene));
}
