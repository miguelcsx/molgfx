//! Occlusion culling of large opaque sphere sets keeps the image.
#![cfg(not(target_arch = "wasm32"))]
use std::fmt::Write;

use molgfx_core::{AtomSelection, RepresentationKind, Scene};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;

const SIDE: u32 = 40;
/// Enough extra atoms to cross the occlusion threshold, far outside the view.
const FAR_ATOMS: u32 = 70_000;

fn pdb(far: u32) -> Vec<u8> {
    let mut text = String::new();
    let mut serial = 0u32;
    let mut push = |x: f64, y: f64, z: f64| {
        serial += 1;
        let _ = writeln!(
            text,
            "HETATM{:5}  C1  UNK A   1    {x:8.3}{y:8.3}{z:8.3}  1.00 10.00           C",
            serial % 100_000
        );
    };
    for i in 0..SIDE {
        for j in 0..SIDE {
            for k in 0..SIDE {
                push(f64::from(i), f64::from(j), f64::from(k));
            }
        }
    }
    // The scene bounds, which set the lighting, must not depend on how many
    // far atoms there are, so the small scene keeps only the two extremes.
    let at = |n: u32| (5000.0 + f64::from(n % 100) * 4.0, f64::from(n / 100) * 4.0);
    let kept: Vec<u32> = if far == FAR_ATOMS {
        (0..far).collect()
    } else {
        vec![0, FAR_ATOMS - 1]
    };
    for n in kept {
        let (x, y) = at(n);
        push(x, y, 0.0);
    }
    text.push_str("END\n");
    text.into_bytes()
}

fn render(far: u32) -> Vec<u8> {
    let (structure, _) =
        molframe::read_bytes(pdb(far), Some("lattice.pdb"), &molframe::ReadOptions::new())
            .expect("lattice parses");
    let mut scene = Scene::from_structure(&structure).expect("scene");
    let all = scene.add_selection(AtomSelection::All);
    scene
        .represent(all, RepresentationKind::Spacefill)
        .expect("form");
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Converged,
            profile: RenderProfile::bare()
                .with_effect(PresentationEffect::Backdrop(BackdropStyle::transparent())),
            ..EngineConfig::default()
        },
        None,
    )
    .expect("GPU opens");
    let camera = Camera {
        eye: Vec3::new(19.5, 19.5, 120.0),
        target: Vec3::new(19.5, 19.5, 19.5),
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 48.0,
            aspect: 1.0,
            near: 0.1,
            far: 200.0,
        },
    };
    engine
        .render_image(
            &scene,
            &camera,
            ImageConfig {
                width: 256,
                height: 256,
            },
        )
        .expect("render")
        .pixels
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn occlusion_culling_of_a_large_sphere_set_keeps_the_image() {
    let plain = render(0);
    let culled = render(FAR_ATOMS);
    assert!(plain.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));
    let differing = plain
        .iter()
        .zip(&culled)
        .filter(|(a, b)| a.abs_diff(**b) > 8)
        .count();
    assert!(
        differing * 200 < plain.len(),
        "{differing} of {} channels differ beyond tolerance",
        plain.len()
    );
}
