//! End-to-end checks of deep scalar crossings and empty isosurfaces.

#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{Representation, ScalarVolume, Scene, VolumeStyle};
use molgfx_math::{Camera, Mat4, Projection, Quat, Vec3};
use molgfx_render::{Engine, EngineConfig, ImageConfig, RenderMode};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn scene(level: f32, scale: f32, crossing: usize, occupy_bricks: bool) -> Scene {
    let mut values = vec![0.0; 1001 * 3 * 3];
    // Off-axis density keeps every macrocell occupied without introducing
    // a crossing on the camera's central rays.
    if occupy_bricks {
        values[..1001].fill(1.0);
    }
    for z in 0..3 {
        for y in 0..3 {
            values[crossing + 1001 * (y + 3 * z)] = 1.0;
        }
    }
    let volume = ScalarVolume::new(
        [1001, 3, 3],
        Mat4::from_scale_rotation_translation(
            Vec3::new(scale, 100.0, 100.0),
            Quat::default(),
            Vec3::ZERO,
        ),
        Arc::from(values),
    )
    .unwrap();
    let mut scene = Scene::new();
    let handle = scene.add_volume(volume);
    scene
        .represent(
            handle,
            Representation::volume()
                .volume_style(VolumeStyle::isosurface().sampling(1.0, 0.65))
                .isolevel(level),
        )
        .unwrap();
    scene
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn deep_crossings_render_and_absent_crossings_match_an_empty_scene() {
    let camera = Camera {
        eye: Vec3::new(-10.0, 100.0, 100.0),
        target: Vec3::new(500.0, 100.0, 100.0),
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 50.0,
            aspect: 1.0,
            near: 0.1,
            far: 2000.0,
        },
    };
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            ..EngineConfig::default()
        },
        None,
    )
    .unwrap();
    let config = ImageConfig {
        width: 96,
        height: 96,
    };
    let empty = engine.render_image(&Scene::new(), &camera, config).unwrap();
    let absent = engine
        .render_image(&scene(2.0, 1.0, 900, true), &camera, config)
        .unwrap();
    assert_eq!(absent.pixels, empty.pixels);
    let crossing = engine
        .render_image(&scene(0.5, 1.0, 900, true), &camera, config)
        .unwrap();
    let changed = crossing
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(empty.pixels.as_chunks::<4>().0.iter())
        .filter(|(actual, background)| actual != background)
        .count();
    assert!(
        changed > 96 * 96 / 2,
        "deep surface must cover the image: {changed}"
    );
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn small_affine_voxels_do_not_lose_thin_isosurface_crossings() {
    let scale = 1.0e-6;
    let camera = Camera {
        eye: Vec3::new(-10.0 * scale, 100.0, 100.0),
        target: Vec3::new(500.0 * scale, 100.0, 100.0),
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 50.0,
            aspect: 1.0,
            near: 0.1 * scale,
            far: 2000.0 * scale,
        },
    };
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            ..EngineConfig::default()
        },
        None,
    )
    .unwrap();
    let config = ImageConfig {
        width: 96,
        height: 96,
    };
    let empty = engine.render_image(&Scene::new(), &camera, config).unwrap();
    for occupy_bricks in [true, false] {
        let crossing = engine
            .render_image(&scene(0.5, scale, 901, occupy_bricks), &camera, config)
            .unwrap();
        let changed = crossing
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .zip(empty.pixels.as_chunks::<4>().0.iter())
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            changed > 96 * 96 / 2,
            "small affine surface must cover the image: {changed}"
        );
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_strongly_sheared_affine_preserves_interior_density_crossings() {
    let mut values = vec![0.0; 27];
    for z in 0..3 {
        for y in 0..3 {
            values[1 + 3 * (y + 3 * z)] = 1.0;
        }
    }
    let transform = Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0, 1.0, 1.0e-6, 0.0, 0.0, 0.0, 0.0, 100.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);
    let mut scene = Scene::new();
    let volume = scene.add_volume(ScalarVolume::new([3; 3], transform, Arc::from(values)).unwrap());
    scene
        .represent(
            volume,
            Representation::volume()
                .volume_style(VolumeStyle::isosurface())
                .isolevel(0.5),
        )
        .unwrap();
    let camera = Camera {
        eye: Vec3::new(2.0, -1.0e-5, 100.0),
        target: Vec3::new(2.0, 1.0e-6, 100.0),
        up: Vec3::Z,
        projection: Projection::Orthographic {
            height: 1.0,
            aspect: 1.0,
            near: 1.0e-7,
            far: 1.0e-3,
        },
    };
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            ..EngineConfig::default()
        },
        None,
    )
    .unwrap();
    let config = ImageConfig {
        width: 96,
        height: 96,
    };
    let empty = engine.render_image(&Scene::new(), &camera, config).unwrap();
    let crossing = engine.render_image(&scene, &camera, config).unwrap();
    let changed = crossing
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(empty.pixels.as_chunks::<4>().0.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed > 96 * 96 / 2,
        "sheared surface must cover the image: {changed}"
    );
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn constant_fields_have_no_isosurface_crossing_at_any_level() {
    let camera = Camera {
        eye: Vec3::new(0.5, 0.5, 5.0),
        target: Vec3::splat(0.5),
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 0.5,
            aspect: 1.0,
            near: 0.1,
            far: 10.0,
        },
    };
    let mut engine = Engine::<WgpuDevice>::new(&EngineConfig::default(), None).unwrap();
    let config = ImageConfig {
        width: 32,
        height: 32,
    };
    let empty = engine.render_image(&Scene::new(), &camera, config).unwrap();
    for level in [0.0, 1.0, 2.0] {
        let mut scene = Scene::new();
        let volume = ScalarVolume::new([2; 3], Mat4::IDENTITY, Arc::from([1.0; 8])).unwrap();
        let handle = scene.add_volume(volume);
        scene
            .represent(
                handle,
                Representation::volume()
                    .volume_style(VolumeStyle::isosurface())
                    .isolevel(level),
            )
            .unwrap();
        let image = engine.render_image(&scene, &camera, config).unwrap();
        assert!(
            image.pixels == empty.pixels,
            "constant field at level {level}"
        );
    }
}
