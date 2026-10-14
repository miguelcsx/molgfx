//! Shared analytical molecular triangles on the native GPU.
#![cfg(not(target_arch = "wasm32"))]
use molgfx_core::{
    AtomSelection, MolecularSurfaceGeometry, RepresentationConfig, RepresentationKind, Scene,
    SurfaceStyle,
};
use molgfx_math::{Camera, Projection, Rgba8, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

#[test]
#[ignore = "requires a native GPU adapter"]
fn analytical_triangles_keep_atom_colors_and_switch_between_solid_and_real_triangle_edges() {
    let (mut scene, handle) = surface_scene();
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
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 20.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 4.0,
            aspect: 1.0,
            near: 0.1,
            far: 40.0,
        },
    };
    let image = ImageConfig {
        width: 128,
        height: 128,
    };
    let solid = engine.render_image(&scene, &camera, image).expect("solid");
    let picked = engine
        .pick(64, 64)
        .expect("pick readback")
        .expect("molecular triangle");
    assert!(
        matches!(picked.entity, molgfx_render::PickEntity::Structure(identity)
        if identity.kind() == molgfx_core::EntityKind::Atom && identity.row() == molgfx_core::LogicalRow::new(0))
    );
    let solid_coverage = coverage(&solid.pixels);
    assert!(solid_coverage > 3000);
    assert!(
        solid
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] != 0)
            .all(|pixel| u16::from(pixel[0]) > u16::from(pixel[1]) * 2
                && u16::from(pixel[0]) > u16::from(pixel[2]) * 2)
    );
    scene
        .representation_mut(handle)
        .expect("representation")
        .params
        .surface_style = SurfaceStyle::Mesh;
    let wire = engine.render_image(&scene, &camera, image).expect("wire");
    let wire_coverage = coverage(&wire.pixels);
    assert!(
        wire_coverage > 100 && wire_coverage < solid_coverage / 4,
        "{solid_coverage} vs {wire_coverage}"
    );
    assert_eq!(
        wire.pixels[(64 * 128 + 64) * 4 + 3],
        0,
        "triangle interior stays empty"
    );
    scene
        .representation_mut(handle)
        .expect("representation")
        .color = molgfx_core::ColorScheme::Uniform(Rgba8::opaque(20, 30, 240));
    let blue = engine
        .render_image(&scene, &camera, image)
        .expect("recolor");
    assert!(
        blue.pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] != 0)
            .all(|pixel| u16::from(pixel[2]) > u16::from(pixel[0]) * 2)
    );
    scene
        .representation_mut(handle)
        .expect("representation")
        .params
        .surface_style = SurfaceStyle::Solid;
    assert_eq!(
        coverage(
            &engine
                .render_image(&scene, &camera, image)
                .expect("solid restored")
                .pixels
        ),
        solid_coverage
    );
}

fn coverage(pixels: &[u8]) -> usize {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] != 0)
        .count()
}

fn surface_scene() -> (Scene, molgfx_core::RepresentationHandle) {
    let pdb =
        b"HETATM    1  O   HOH A   1       0.000   0.000   0.000  1.00 10.00           O\nEND\n";
    let (source, _) = molframe::read_bytes(
        pdb.to_vec(),
        Some("surface.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source");
    let mut scene = Scene::from_structure(&source).expect("scene");
    let mut geometry = molframe::surface::IndexedSurfaceMesh::new(
        vec![[-1.5, -1.5, 0.0], [1.5, -1.5, 0.0], [0.0, 1.5, 0.0]],
        vec![molframe::surface::SurfaceFace([0, 1, 2])],
    );
    geometry.vertex_normals = vec![[0.0, 0.0, 1.0]; 3];
    let surface =
        MolecularSurfaceGeometry::new(Arc::new(molframe::surface::MolecularSurfaceMesh {
            geometry,
            atom_rows: vec![0; 3].into_boxed_slice(),
            discarded_degenerate_faces: 0,
        }))
        .expect("analytical geometry");
    let selection = scene.add_selection(AtomSelection::All);
    let handle = scene
        .represent(
            selection,
            RepresentationConfig::new(RepresentationKind::MolecularSurface)
                .molecular_surface(surface)
                .line_width(2.0),
        )
        .expect("representation");
    (scene, handle)
}

#[test]
#[ignore = "requires a native GPU adapter and caller-installed Surf and MSMS"]
fn authentic_native_providers_render_their_analytic_sphere_and_original_atom_identity() {
    use molframe::surface::native_solver::{
        NativeSurfaceOptions, NativeSurfaceProvider, SurfaceAtoms, native_molecular_surface,
    };
    use molframe::{ExecutionContext, TempStoragePolicy};
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
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 20.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 4.0,
            aspect: 1.0,
            near: 0.1,
            far: 40.0,
        },
    };
    let context = ExecutionContext::builder()
        .temp_storage_policy(TempStoragePolicy::directory(
            std::env::temp_dir(),
            1_000_000_000,
        ))
        .build()
        .expect("context");
    for (variable, provider) in [
        ("MOLFRAME_SURF", NativeSurfaceProvider::Surf),
        ("MOLFRAME_MSMS", NativeSurfaceProvider::Msms),
    ] {
        let executable = std::env::var(variable).expect("explicit scientific provider path");
        let output = native_molecular_surface(
            std::path::Path::new(&executable),
            provider,
            SurfaceAtoms {
                positions: &[[0.0; 3]],
                radii: &[1.52],
                rows: &[0],
            },
            NativeSurfaceOptions::default(),
            &context,
        )
        .expect("actual provider");
        let (source, _) = molframe::read_bytes(
            b"HETATM    1  O   HOH A   1       0.000   0.000   0.000  1.00 10.00           O\nEND\n".to_vec(),
            Some("oxygen.pdb"), &molframe::ReadOptions::new()).expect("molecule");
        let mut scene = Scene::from_structure(&source).expect("scene");
        let selection = scene.add_selection(AtomSelection::All);
        scene
            .represent(
                selection,
                RepresentationConfig::new(RepresentationKind::MolecularSurface).molecular_surface(
                    MolecularSurfaceGeometry::new(Arc::new(output)).expect("asset"),
                ),
            )
            .expect("representation");
        let image = engine
            .render_image(
                &scene,
                &camera,
                ImageConfig {
                    width: 128,
                    height: 128,
                },
            )
            .expect("scientific geometry on GPU");
        let area = f64::from(u32::try_from(coverage(&image.pixels)).expect("bounded image"));
        let analytic = std::f64::consts::PI * (1.52_f64 * 32.0).powi(2);
        assert!(
            area > analytic * 0.8 && area < analytic * 1.05,
            "{provider:?} area {area} analytic {analytic}"
        );
        let picked = engine.pick(64, 64).expect("pick").expect("surface");
        assert!(
            matches!(picked.entity, molgfx_render::PickEntity::Structure(identity)
            if identity.kind() == molgfx_core::EntityKind::Atom && identity.row() == molgfx_core::LogicalRow::new(0))
        );
    }
}

#[test]
#[ignore = "requires a native GPU adapter and caller-installed Surf and MSMS"]
fn authentic_analytical_samples_render_at_the_calculated_pose_and_refuse_stale_cached_draws() {
    use molframe::surface::native_solver::NativeSurfaceProvider;
    let context = molframe::ExecutionContext::builder()
        .temp_storage_policy(molframe::TempStoragePolicy::directory(
            std::env::temp_dir(),
            1_000_000_000,
        ))
        .build()
        .expect("context");
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
    for (variable, provider) in [
        ("MOLFRAME_SURF", NativeSurfaceProvider::Surf),
        ("MOLFRAME_MSMS", NativeSurfaceProvider::Msms),
    ] {
        let (mut scene, owner) = sampled_provider_scene(variable, provider, &context);
        let image = ImageConfig {
            width: 128,
            height: 128,
        };
        let first = engine
            .render_image(&scene, &camera, image)
            .expect("sample on GPU");
        assert!(coverage(&first.pixels) > 1500);
        let picked = engine
            .pick(80, 64)
            .expect("readback")
            .expect("sampled sphere");
        assert!(
            matches!(picked.entity, molgfx_render::PickEntity::Structure(identity)
            if identity.row() == molgfx_core::LogicalRow::new(0))
        );
        engine
            .render_image(&scene, &camera, image)
            .expect("cached unchanged sample");
        scene.set_trajectory_time(owner, 0.5).expect("advance");
        let stale = engine
            .render_image(&scene, &camera, image)
            .expect("playback continues without stale geometry");
        assert_eq!(
            coverage(&stale.pixels),
            0,
            "{provider:?} must omit a stale cached draw"
        );
        assert!(
            engine.pick(80, 64).expect("readback").is_none(),
            "hidden stale geometry cannot be picked"
        );
        scene.set_trajectory_time(owner, 0.25).expect("reverse");
        let restored = engine
            .render_image(&scene, &camera, image)
            .expect("original calculated sample");
        assert!(
            first.pixels == restored.pixels,
            "same scientific sample restores its image"
        );
    }
}

fn sampled_provider_scene(
    variable: &str,
    provider: molframe::surface::native_solver::NativeSurfaceProvider,
    context: &molframe::ExecutionContext,
) -> (Scene, molgfx_core::StructureHandle) {
    use molframe::surface::native_solver::{
        NativeSurfaceOptions, SurfaceAtoms, SurfaceCoordinateSample,
        native_molecular_surface_at_sample,
    };
    use molgfx_core::{MolecularPose, TrajectoryFrame, TrajectorySegment};
    let (mut scene, representation) = surface_scene();
    let owner = scene.structures().next().expect("source").0;
    scene
        .set_trajectory_segment(
            owner,
            TrajectorySegment::new(
                TrajectoryFrame::new(0, 0.0, Arc::from([[0.0; 3]]), "test").expect("start"),
                TrajectoryFrame::new(1, 1.0, Arc::from([[4.0, 0.0, 0.0]]), "test").expect("end"),
                0.25,
            )
            .expect("segment"),
        )
        .expect("bind");
    let pose = MolecularPose::capture(scene.structure(owner).expect("placed"));
    let output = native_molecular_surface_at_sample(
        std::path::Path::new(&std::env::var(variable).expect("explicit provider")),
        provider,
        SurfaceAtoms {
            positions: pose.positions(),
            radii: &[1.52],
            rows: &[0],
        },
        SurfaceCoordinateSample {
            end_positions: pose.end_positions().expect("end"),
            weight: molgfx_core::narrow_to_f32(pose.interpolation().expect("weight"))
                .expect("weight is representable as f32"),
        },
        NativeSurfaceOptions::default(),
        context,
    )
    .expect("actual sampled provider");
    scene
        .representation_mut(representation)
        .expect("representation")
        .molecular_surface = Some(
        MolecularSurfaceGeometry::new(Arc::new(output))
            .expect("geometry")
            .for_pose(&pose),
    );
    (scene, owner)
}
