//! End-to-end colour-space checks of unlit density slices.

#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{
    ClipPlane, Representation, ScalarVolume, Scene, VolumeSlice, VolumeStyle,
    VolumeTransferFunction, VolumeTransferPoint,
};
use molgfx_math::{Camera, Mat4, Projection, Rgba8, Vec3};
use molgfx_render::{Engine, EngineConfig, ImageConfig, RenderMode};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

#[test]
#[ignore = "requires a native GPU adapter"]
fn slice_colours_enter_the_hdr_buffer_in_scene_linear_space() {
    check_slice_colour(false);
    check_slice_colour(true);
}

fn check_slice_colour(exact_palette: bool) {
    let volume = ScalarVolume::new([2; 3], Mat4::IDENTITY, Arc::from([1.0; 8])).unwrap();
    let plane = ClipPlane::from_point_normal(Vec3::splat(0.5), Vec3::Z).unwrap();
    let gray = Rgba8::opaque(128, 128, 128);
    let transfer = VolumeTransferFunction::new(&[
        VolumeTransferPoint::new(0.0, gray, 1.0),
        VolumeTransferPoint::new(1.0, gray, 1.0),
    ])
    .unwrap();
    let mut style = VolumeStyle::slice(VolumeSlice::new(plane))
        .transfer(transfer)
        .sampling(1.0, 0.65);
    if exact_palette {
        let mut colors = [Rgba8::opaque(200, 200, 200); 10];
        colors[9] = gray;
        style = style.slice_ramp(molgfx_core::ScalarRamp::evenly([0.0, 1.0], &colors).unwrap());
    }
    let mut scene = Scene::new();
    let volume = scene.add_volume(volume);
    let representation = scene
        .represent(volume, Representation::volume().volume_style(style))
        .unwrap();
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
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            ..EngineConfig::default()
        },
        None,
    )
    .unwrap();
    let image = engine
        .render_hdr_image(
            &scene,
            &camera,
            ImageConfig {
                width: 32,
                height: 32,
            },
        )
        .unwrap();
    let center = &image.rgba16f()[(16 * 32 + 16) * 8..][..8];
    for channel in 0..3 {
        let value = u16::from_le_bytes([center[channel * 2], center[channel * 2 + 1]]);
        // Linear IEC sRGB 128/255 is 0.2158605 (binary16 0x32e8).
        // Allow the shared shader approximation and half-float rounding.
        assert!(
            value.abs_diff(0x32e8) <= 16,
            "HDR channel is not linear: {value:#06x}"
        );
    }
    assert_eq!(u16::from_le_bytes([center[6], center[7]]), 0x3c00);
    check_hdr_coverage(&mut engine, &mut scene, representation, &camera);
}

fn check_hdr_coverage(
    engine: &mut Engine<WgpuDevice>,
    scene: &mut Scene,
    representation: molgfx_core::RepresentationHandle,
    camera: &Camera,
) {
    engine
        .set_render_profile(
            molgfx_render::RenderProfile::bare().with_effect(
                molgfx_render::PresentationEffect::Backdrop(
                    molgfx_render::BackdropStyle::transparent(),
                ),
            ),
        )
        .unwrap();
    let empty = engine
        .render_hdr_image(
            &Scene::new(),
            camera,
            ImageConfig {
                width: 32,
                height: 32,
            },
        )
        .unwrap();
    assert!(
        empty
            .rgba16f()
            .as_chunks::<8>()
            .0
            .iter()
            .all(|pixel| pixel[6..8] == [0, 0])
    );
    scene
        .representation_mut(representation)
        .unwrap()
        .material
        .opacity = 0.5;
    let translucent = engine
        .render_hdr_image(
            scene,
            camera,
            ImageConfig {
                width: 32,
                height: 32,
            },
        )
        .unwrap();
    // Material opacity uses an 8-bit upload: 0.5 becomes 127/255.
    let alpha = &translucent.rgba16f()[(16 * 32 + 16) * 8 + 6..][..2];
    assert!(
        u16::from_le_bytes([alpha[0], alpha[1]]).abs_diff(0x37f8) <= 2,
        "alpha: {:#06x}",
        u16::from_le_bytes([alpha[0], alpha[1]])
    );
}
