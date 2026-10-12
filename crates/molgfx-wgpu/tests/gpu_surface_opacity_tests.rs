//! Translucent implicit surfaces blend over the geometry behind them.
#![cfg(not(target_arch = "wasm32"))]
use molgfx_core::{
    AtomSelection, ColorScheme, RepresentationConfig, RepresentationKind, Scene, SurfaceKind,
};
use molgfx_math::{Camera, Projection, Rgba8, Vec3};
use molgfx_render::{Engine, EngineConfig, ImageConfig, RenderMode, RenderProfile};
use molgfx_wgpu::WgpuDevice;

const SIZE: u32 = 96;
const CENTER: usize = (SIZE as usize / 2 * SIZE as usize + SIZE as usize / 2) * 4;

/// A blue sphere sits behind a red surface seen head-on.
fn render(mode: RenderMode, kind: SurfaceKind, opacity: f32) -> Vec<u8> {
    let pdb = b"HETATM    1  N   LIG A   1       0.000   0.000  -4.000  1.00 10.00           N\n\
HETATM    2  O   HOH A   2       0.000   0.000   0.000  1.00 10.00           O\nEND\n";
    let (source, _) = molframe::read_bytes(
        pdb.to_vec(),
        Some("surface.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source");
    let mut scene = Scene::from_structure(&source).expect("scene");
    let behind = scene.add_selection(AtomSelection::Range(0..1));
    let front = scene.add_selection(AtomSelection::Range(1..2));
    scene
        .represent(
            behind,
            RepresentationConfig::new(RepresentationKind::Spacefill)
                .color(ColorScheme::Uniform(Rgba8::opaque(20, 30, 240))),
        )
        .expect("sphere");
    let handle = scene
        .represent(
            front,
            RepresentationConfig::new(RepresentationKind::Surface)
                .color(ColorScheme::Uniform(Rgba8::opaque(240, 20, 20))),
        )
        .expect("surface");
    let representation = scene.representation_mut(handle).expect("representation");
    representation.params.surface_kind = kind;
    representation.material.opacity = opacity;
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode,
            profile: RenderProfile::bare(),
            ..EngineConfig::default()
        },
        None,
    )
    .expect("GPU");
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
            &scene,
            &camera,
            ImageConfig {
                width: SIZE,
                height: SIZE,
            },
        )
        .expect("image")
        .pixels
}

fn assert_reveals_geometry_behind(mode: RenderMode, kind: SurfaceKind) {
    let opaque = render(mode, kind, 1.0);
    let glass = render(mode, kind, 0.3);
    let at = |pixels: &[u8]| [pixels[CENTER], pixels[CENTER + 2], pixels[CENTER + 3]];
    let [opaque_red, opaque_blue, opaque_alpha] = at(&opaque);
    let [glass_red, glass_blue, glass_alpha] = at(&glass);
    assert_eq!(opaque_alpha, 255, "an opaque surface covers the pixel");
    assert!(
        opaque_red > opaque_blue,
        "an opaque surface hides the sphere behind it: {opaque_red} {opaque_blue}"
    );
    assert!(
        glass_blue > opaque_blue + 60 && glass_blue > glass_red / 2,
        "a translucent surface shows the sphere behind it: {glass_red} {glass_blue}"
    );
    assert!(glass_red > 20, "the surface itself still tints the pixel");
    assert_eq!(glass_alpha, 255, "the sphere behind keeps coverage");
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn translucent_solvent_excluded_surface_reveals_geometry_behind_it_in_realtime() {
    assert_reveals_geometry_behind(RenderMode::Realtime, SurfaceKind::SolventExcluded);
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn translucent_van_der_waals_surface_reveals_geometry_behind_it_in_realtime() {
    assert_reveals_geometry_behind(RenderMode::Realtime, SurfaceKind::VanDerWaals);
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn translucent_gaussian_surface_reveals_geometry_behind_it_in_realtime() {
    assert_reveals_geometry_behind(RenderMode::Realtime, SurfaceKind::Gaussian);
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn translucent_solvent_excluded_surface_reveals_geometry_behind_it_when_converged() {
    assert_reveals_geometry_behind(RenderMode::Converged, SurfaceKind::SolventExcluded);
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn translucent_van_der_waals_surface_reveals_geometry_behind_it_when_converged() {
    assert_reveals_geometry_behind(RenderMode::Converged, SurfaceKind::VanDerWaals);
}
