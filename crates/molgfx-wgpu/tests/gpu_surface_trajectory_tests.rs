//! Sampled surfaces follow live coordinates while retaining their field resources.
#![cfg(not(target_arch = "wasm32"))]
use molgfx_core::{
    AtomSelection, RepresentationKind, Scene, SurfaceKind, TrajectoryFrame, TrajectorySegment,
};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn source() -> molframe::Structure {
    let pdb = b"ATOM      1  CA  ALA A   1      -4.000   0.000   0.000  1.00 10.00           C\nATOM      2  O   ALA A   1      -4.000   1.500   0.000  1.00 10.00           O\nATOM      3  CA  ALA A   2       0.000   0.000   0.000  1.00 10.00           C\nATOM      4  O   ALA A   2       0.000   1.500   0.000  1.00 10.00           O\nATOM      5  CA  ALA A   3       4.000   0.000   0.000  1.00 10.00           C\nATOM      6  O   ALA A   3       4.000   1.500   0.000  1.00 10.00           O\nEND\n";
    molframe::read_bytes(
        pdb.to_vec(),
        Some("ribbons.pdb"),
        &molframe::ReadOptions::new(),
    )
    .unwrap()
    .0
}

fn pixels(engine: &mut Engine<WgpuDevice>, scene: &Scene) -> Vec<u8> {
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 20.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 12.0,
            aspect: 1.0,
            near: 0.1,
            far: 40.0,
        },
    };
    engine
        .render_image(
            scene,
            &camera,
            ImageConfig {
                width: 256,
                height: 256,
            },
        )
        .expect("ribbons render")
        .pixels
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn sampled_surfaces_follow_pose_changes_and_restore_the_original_field() {
    let source = source();
    let flat: Arc<[[f32; 3]]> = Arc::from(
        source
            .atoms()
            .into_iter()
            .map(|atom| atom.position().expect("coordinates"))
            .collect::<Vec<_>>(),
    );
    let moved: Arc<[[f32; 3]]> = Arc::from(
        flat.iter()
            .map(|point| [point[0], point[1] + 3.0, point[2]])
            .collect::<Vec<_>>(),
    );
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
    for kind in [
        SurfaceKind::Gaussian,
        SurfaceKind::SolventAccessible,
        SurfaceKind::SolventExcluded,
    ] {
        let mut scene = Scene::from_structure(&source).expect("scene");
        let owner = scene.structures().next().expect("placed source").0;
        let selection = scene.add_selection(AtomSelection::All);
        let representation = scene
            .represent(selection, RepresentationKind::Surface)
            .expect("surface");
        scene
            .representation_mut(representation)
            .expect("surface controls")
            .params
            .surface_kind = kind;
        let static_pixels = pixels(&mut engine, &scene);
        let static_coverage = static_pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] != 0)
            .count();
        assert!(
            static_coverage > 100,
            "{kind:?} static boundary coverage {static_coverage}"
        );
        let segment = TrajectorySegment::new(
            TrajectoryFrame::new(0, 0.0, Arc::clone(&flat), "initial").expect("start"),
            TrajectoryFrame::new(1, 1.0, Arc::clone(&moved), "moved").expect("end"),
            0.0,
        )
        .expect("segment");
        scene
            .set_trajectory_segment(owner, segment)
            .expect("trajectory");
        let initial = pixels(&mut engine, &scene);
        assert!(
            initial
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| pixel[3] != 0)
                .count()
                > 100,
            "{kind:?} must draw a boundary, static coverage {static_coverage}"
        );
        assert!(
            initial == pixels(&mut engine, &scene),
            "{kind:?} is stable without motion"
        );
        scene
            .set_trajectory_time(owner, 1.0)
            .expect("interpolation");
        let shifted = pixels(&mut engine, &scene);
        assert!(initial != shifted, "{kind:?} must sample the new pose");
        scene.set_trajectory_time(owner, 0.0).expect("reverse");
        assert!(
            initial == pixels(&mut engine, &scene),
            "{kind:?} must restore its field"
        );
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_single_gaussian_boundary_matches_its_analytic_radius_within_four_percent_area() {
    let source = source();
    let mut scene = Scene::from_structure(&source).expect("scene");
    let selection = scene.add_selection(AtomSelection::Range(2..3));
    let handle = scene
        .represent(selection, RepresentationKind::Surface)
        .expect("Gaussian");
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
    for (scale, sigma, level, cutoff) in [
        (1.0f32, 1.0f32, 0.5f32, 4.0),
        (1.5, 0.7, 0.2, 5.0),
        (1.0, 1.0, 0.01, 4.0),
    ] {
        let params = &mut scene
            .representation_mut(handle)
            .expect("density controls")
            .params;
        params.surface_kind = SurfaceKind::Gaussian;
        params.radius_scale = scale;
        params.gaussian_sigma = sigma;
        params.isolevel = level;
        params.gaussian_cutoff = cutoff;
        params.surface_spacing = Some(0.25);
        let image = pixels(&mut engine, &scene);
        let area: f64 = image
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| f64::from(pixel[3]) / 255.0)
            .sum();
        let radius = 1.7 * scale * sigma * (-2.0 * level.ln()).sqrt();
        let expected_area = std::f64::consts::PI * f64::from(radius * 256.0 / 12.0).powi(2);
        let error = (area - expected_area).abs() / expected_area;
        assert!(
            error < 0.04,
            "scale {scale}, sigma {sigma}, density {level}: measured {area}, expected {expected_area}, error {error}"
        );
    }
}
