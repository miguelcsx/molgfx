//! Real-device shader validation for the colour and specialization paths.
//!
//! The rest of the suite runs against a mock device. The mock proves the
//! bookkeeping — that the colour block carries the right selector, that records
//! stay element-coloured, that a scheme change writes only fixed-size uniforms —
//! but it cannot prove the WGSL is valid, because it never creates a pipeline.
//! This does, on the actual wgpu backend, for every colour scheme the engine
//! exposes. That check has real value: it is what caught the representation
//! layout declaring the colour block for the fragment stage alone while the
//! vertex stage of the sphere and bond pipelines reads it.
//!
//! # What this does not prove
//!
//! It does not compare rendered pixels between schemes. The offscreen path
//! produces no molecular geometry for this fixture — an empty scene and a
//! three-atom scene render pixel-identical frames whose darkest pixel is the
//! backdrop — so a comparison of two frames here would compare two backdrops and
//! pass vacuously. Colour correctness is covered by the mock-level tests named
//! above, which assert the block's contents rather than a framebuffer.
//!
//! Ignored by default because it needs a native GPU adapter; run it with
//! `cargo test -p molgfx-wgpu --all-features -- --ignored`.

#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{AtomSelection, ColorScheme, RepresentationKind, Scene, VisualStyle};
use molgfx_math::{BoundingSphere, Camera, Rgba8, Vec3};
use molgfx_render::{Engine, EngineConfig, ImageConfig, RenderMode};
use molgfx_wgpu::WgpuDevice;

const WIDTH: u32 = 96;
const HEIGHT: u32 = 96;

fn camera() -> Camera {
    Camera::framing(
        &BoundingSphere {
            center: Vec3::new(1.5, 0.5, 0.0),
            radius: 3.0,
        },
        1.0,
    )
}

fn structure() -> molframe::Structure {
    let cif = "\
data_test
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_alt_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
_atom_site.auth_seq_id
_atom_site.auth_asym_id
_atom_site.pdbx_PDB_model_num
ATOM 1 N N  . GLY A 1 1 0.0 0.0 0.0 1.00 10.0 1 A 1
ATOM 2 C CA . GLY A 1 1 1.5 0.0 0.0 1.00 10.0 1 A 1
ATOM 3 O O  . GLY A 1 1 3.0 1.0 0.0 1.00 10.0 1 A 1
";
    match molframe::read_bytes(
        cif.as_bytes().to_vec(),
        Some("colour-test.cif"),
        &molframe::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("fixture parses: {diagnostics:?}"),
    }
}

/// A scene with two spacefill representations sharing one selection.
///
/// Two representations over one selection is the configuration every sharing
/// change in the engine exists to serve, so it is also the one a real device
/// should be asked to compile pipelines for.
fn scene(scheme: ColorScheme, style: Option<&VisualStyle>) -> Scene {
    let mut scene = Scene::new();
    let source = structure();
    if let Err(error) = scene.add_structure(&source) {
        panic!("fixture structure places: {error}");
    }
    let selection = scene.add_selection(AtomSelection::All);
    for order in 0..2 {
        let Ok(handle) = scene.represent(selection, RepresentationKind::Spacefill) else {
            panic!("spacefill applies")
        };
        let Some(representation) = scene.representation_mut(handle) else {
            panic!("representation resolves")
        };
        // Only used to give the two representations distinct draw orders.
        representation.order = u16::from(order != 0);
        representation.color = scheme;
        representation.visual = style.cloned();
    }
    scene
}

/// Renders two frames and an offscreen image, failing on any device error.
///
/// Two frames settle anything the first defers, so the image is what a caller
/// sees once every pipeline exists.
fn render(scene: &Scene) -> molgfx_render::Image {
    let config = EngineConfig {
        mode: RenderMode::Realtime,
        ..EngineConfig::default()
    };
    let mut engine = match Engine::<WgpuDevice>::new(&config, None) {
        Ok(engine) => engine,
        Err(error) => panic!("headless engine opens: {error}"),
    };
    for _ in 0..2 {
        if let Err(error) = engine.render(scene, &camera()) {
            panic!("frame renders: {error}");
        }
    }
    match engine.render_image(
        scene,
        &camera(),
        ImageConfig {
            width: WIDTH,
            height: HEIGHT,
        },
    ) {
        Ok(image) => image,
        Err(error) => panic!("image renders: {error}"),
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn every_colour_scheme_builds_its_pipelines_on_a_real_device() {
    // Each scheme selects different generated code, so each has to survive real
    // pipeline validation. A binding the shader stage needs and the layout does
    // not declare surfaces here and nowhere else in the suite.
    for scheme in [
        ColorScheme::ByElement,
        ColorScheme::ByChain,
        ColorScheme::ByResidue,
        ColorScheme::BySecondaryStructure,
        ColorScheme::Uniform(Rgba8::opaque(12, 34, 56)),
    ] {
        let image = render(&scene(scheme, None));
        assert_eq!(
            (image.width, image.height),
            (WIDTH, HEIGHT),
            "{scheme:?} produced a frame"
        );
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_styled_scene_builds_its_pipelines_on_a_real_device() {
    // A typed style selects the visual-program path, whose fragment unit
    // carries the specialization marker, so this is the validation that proves
    // the generated sibling compiles alongside everything that reads it.
    let mut builder = molgfx_core::VisualProgramBuilder::new();
    let Ok(opacity) = builder.base_opacity() else {
        panic!("opacity input builds")
    };
    let Ok(color) = builder.color([0.2, 0.6, 0.9, 1.0]) else {
        panic!("color builds")
    };
    assert!(
        builder.set_opacity(opacity).is_ok() && builder.set_base_color(color).is_ok(),
        "the style's outputs set"
    );
    let Ok(program) = builder.finish() else {
        panic!("style builds")
    };
    let style = VisualStyle::new(program);
    let image = render(&scene(ColorScheme::ByElement, Some(&style)));
    assert_eq!((image.width, image.height), (WIDTH, HEIGHT));
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_surface_scene_builds_its_pipelines_on_a_real_device() {
    // The surface pass reads the BVH from its vertex stage, which is the one
    // stage that forced the representation layout to keep binding 6 visible to
    // every stage rather than narrowing it.
    // The scene helper already added a selection for its spacefill pair, so the
    // surface reuses the same rows.
    let mut scene = scene(ColorScheme::ByChain, None);
    let selection = match scene.representations().next() {
        Some((representation, _)) => match scene.representation(representation) {
            Some(representation) => match representation.selection() {
                Some(handle) => handle,
                None => panic!("the fixture representation names a selection"),
            },
            None => panic!("representation resolves"),
        },
        None => panic!("fixture has a representation"),
    };
    let Ok(handle) = scene.represent(selection, RepresentationKind::Surface) else {
        panic!("surface applies")
    };
    let Some(representation) = scene.representation_mut(handle) else {
        panic!("representation resolves")
    };
    representation.params.surface_kind = molgfx_core::SurfaceKind::SolventAccessible;
    let image = render(&scene);
    assert_eq!((image.width, image.height), (WIDTH, HEIGHT));
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn the_first_frame_pays_the_specialization_stall_once() {
    // Specialization compiles inline on the frame that first needs a style,
    // because a `Device` handle cannot cross to a worker thread without new
    // `Send` bounds on the backend trait. This reports what that costs, which is
    // the one number that decides whether the trade is worth keeping.
    let mut builder = molgfx_core::VisualProgramBuilder::new();
    let Ok(opacity) = builder.base_opacity() else {
        panic!("opacity input builds")
    };
    let Ok(color) = builder.color([0.2, 0.6, 0.9, 1.0]) else {
        panic!("color builds")
    };
    assert!(builder.set_opacity(opacity).is_ok() && builder.set_base_color(color).is_ok());
    let Ok(program) = builder.finish() else {
        panic!("style builds")
    };
    let style = VisualStyle::new(program);
    let scene = scene(ColorScheme::ByElement, Some(&style));

    let config = EngineConfig {
        mode: RenderMode::Realtime,
        ..EngineConfig::default()
    };
    let Ok(mut engine) = Engine::<WgpuDevice>::new(&config, None) else {
        panic!("headless engine opens")
    };
    // One unstyled warm-up so pipeline construction unrelated to style is paid
    // before the measurement starts.
    if let Err(error) = engine.render(&Scene::new(), &camera()) {
        panic!("warm-up renders: {error}");
    }

    let mut frame_times = Vec::new();
    for _ in 0..6 {
        let start = std::time::Instant::now();
        if let Err(error) = engine.render(&scene, &camera()) {
            panic!("frame renders: {error}");
        }
        frame_times.push(start.elapsed());
    }
    let (Some(first), Some(steady)) = (frame_times.first().copied(), frame_times.get(1).copied())
    else {
        panic!("the measurement collected six frames")
    };
    eprintln!(
        "styled first frame {:?}, second frame {:?}, ratio {:.1}x",
        first,
        steady,
        first.as_secs_f64() / steady.as_secs_f64().max(1.0e-9)
    );
    // The stall is real and must stay bounded: one frame, then steady state.
    assert!(
        frame_times.len() == 6,
        "the measurement collected six frames"
    );
    assert!(
        steady.saturating_mul(4) < first,
        "the first styled frame pays a compile the later frames do not"
    );
}
