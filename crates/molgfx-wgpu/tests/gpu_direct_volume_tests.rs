//! End-to-end direct-volume capture and convergence checks.

#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{Representation, ScalarVolume, Scene, VolumeStyle};
use molgfx_math::{Camera, Mat4, Projection, Vec3};
use molgfx_render::{Engine, EngineConfig, ImageConfig, RenderMode};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_converged_direct_volume_has_valid_pixels_after_repeated_outputs() {
    let mut scene = Scene::new();
    let mut values = Vec::with_capacity(113 * 96 * 101);
    for z in 0_u16..101 {
        for y in 0_u16..96 {
            for x in 0_u16..113 {
                let position = Vec3::new(f32::from(x), f32::from(y), f32::from(z));
                let radius = (position - Vec3::new(56.0, 47.5, 50.0)).length();
                values.push(if radius < 30.0 {
                    (-radius * radius / 100.0).exp()
                } else {
                    0.0
                });
            }
        }
    }
    let affine = Mat4::from_cols_array(&[
        0.7,
        0.0,
        0.0,
        0.0,
        0.121_553_72,
        0.689_365_4,
        0.0,
        0.0,
        0.0,
        0.0,
        0.7,
        0.0,
        -34.203_503,
        -19.302_233,
        5.6,
        1.0,
    ]);
    let volume = ScalarVolume::new([113, 96, 101], affine, Arc::from(values)).unwrap();
    let handle = scene.add_volume(volume);
    scene
        .represent(
            handle,
            Representation::volume().volume_style(VolumeStyle::default()),
        )
        .unwrap();
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 160.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Perspective {
            fov_y: std::f32::consts::FRAC_PI_4,
            aspect: 1.0,
            near: 1.0,
            far: 2000.0,
        },
    };
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Converged,
            profile: molgfx_render::RenderProfile::shape_cues(),
            adaptive: molgfx_render::AdaptiveQualityConfig::highest_fixed(1),
            ..EngineConfig::default()
        },
        None,
    )
    .unwrap();
    for _ in 0..3 {
        engine
            .profile_frame(
                &scene,
                &camera,
                ImageConfig {
                    width: 768,
                    height: 768,
                },
                molgfx_render::MeasuredOutput::Converged,
            )
            .unwrap();
        let image = engine
            .render_image(
                &scene,
                &camera,
                ImageConfig {
                    width: 768,
                    height: 768,
                },
            )
            .unwrap();
        assert!(image.quality.complete());
        assert!(
            image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 255)
        );
        assert_ne!(
            &image.pixels[..3],
            &image.pixels[(200 * 768 + 440) * 4..][..3]
        );
    }
}
