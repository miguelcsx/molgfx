//! Buried solvent sample rejection through the resident molecular hierarchy.
#![cfg(not(target_arch = "wasm32"))]
use molgfx_core::{AtomSelection, RepresentationKind, Scene};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;

fn source() -> molframe::Structure {
    let pdb = b"HETATM    1  C1  UNK A   1       0.000   0.000   0.000  1.00 10.00           C\nHETATM    2  O1  UNK A   1       0.000   0.000   0.000  1.00 10.00           O\nEND\n";
    molframe::read_bytes(
        pdb.to_vec(),
        Some("solvent.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source parses")
    .0
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn solvent_rejects_a_fully_buried_sphere_without_changing_its_independent_boundary() {
    let source = source();
    assert_eq!(
        source
            .atoms()
            .into_iter()
            .map(|atom| atom.element().map(molframe::Element::atomic_number))
            .collect::<Vec<_>>(),
        vec![Some(6), Some(8)]
    );
    let make = |kind, selection| {
        let mut scene = Scene::from_structure(&source).expect("scene");
        let selection = scene.add_selection(selection);
        scene.represent(selection, kind).expect("form");
        scene
    };
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
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 20.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 10.0,
            aspect: 1.0,
            near: 0.1,
            far: 40.0,
        },
    };
    let mut image = |scene: &Scene| {
        engine
            .render_image(
                scene,
                &camera,
                ImageConfig {
                    width: 256,
                    height: 256,
                },
            )
            .expect("render")
            .pixels
    };
    let exposed = image(&make(
        RepresentationKind::Solvent,
        AtomSelection::Sparse(vec![0]),
    ));
    let buried = image(&make(RepresentationKind::Solvent, AtomSelection::All));
    assert!(exposed.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));
    assert!(
        exposed == buried,
        "the smaller oxygen sphere is entirely inside the carbon boundary"
    );
    let inner = image(&make(
        RepresentationKind::Solvent,
        AtomSelection::Sparse(vec![1]),
    ));
    assert!(
        inner != exposed,
        "the smaller oxygen boundary renders when selected independently"
    );
    let mut sampled = make(RepresentationKind::Dots, AtomSelection::Sparse(vec![0]));
    let representation = sampled.representations().next().expect("sample layer").0;
    sampled
        .representation_mut(representation)
        .expect("samples edit")
        .params
        .point_samples = 16;
    let coarse = image(&sampled);
    sampled
        .representation_mut(representation)
        .expect("samples edit")
        .params
        .point_samples = 256;
    let dense = image(&sampled);
    let coverage = |pixels: &[u8]| {
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] != 0)
            .count()
    };
    assert!(
        coverage(&dense) > coverage(&coarse) * 4,
        "sample density changes the indirect vertex count on the same scene: coarse {}, dense {}",
        coverage(&coarse),
        coverage(&dense)
    );
}
