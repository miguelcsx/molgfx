//! End-to-end checks of deep scalar crossings and empty isosurfaces.

#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{Representation, ScalarVolume, Scene, VolumeStyle};
use molgfx_math::{Camera, Mat4, Projection, Quat, Vec3};
use molgfx_render::{Engine, EngineConfig, ImageConfig, RenderMode};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn scene(level: f32) -> Scene {
    let mut values = vec![0.0; 1001 * 3 * 3];
    // Off-axis density keeps every macrocell occupied without introducing
    // a crossing on the camera's central rays.
    values[..1001].fill(1.0);
    for z in 0..3 {
        for y in 0..3 {
            values[900 + 1001 * (y + 3 * z)] = 1.0;
        }
    }
    let volume = ScalarVolume::new(
        [1001, 3, 3],
        Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 100.0, 100.0),
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
    let absent = engine.render_image(&scene(2.0), &camera, config).unwrap();
    assert_eq!(absent.pixels, empty.pixels);
    let crossing = engine.render_image(&scene(0.5), &camera, config).unwrap();
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
