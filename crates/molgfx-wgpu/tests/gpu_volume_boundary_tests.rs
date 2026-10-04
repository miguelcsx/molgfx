//! Indexed scalar presentations retain their distinct lattice coverage.
#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{Representation, ScalarVolume, Scene, VolumeRendering, VolumeStyle};
use molgfx_math::{Camera, Mat4, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, Image, ImageConfig, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn coverage(image: &Image) -> usize {
    image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] != 0)
        .count()
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn affine_mesh_and_dots_have_distinct_coverage_and_empty_levels_stay_empty() {
    let transform = Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0, 0.3, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, -2.6, -2.0, -1.0, 1.0,
    ]);
    let values: Arc<[f32]> = (0_u8..5)
        .flat_map(|z| std::iter::repeat_n(f32::from(z), 25))
        .collect();
    let mut scene = Scene::new();
    let volume = scene.add_volume(ScalarVolume::new([5; 3], transform, values).unwrap());
    let handle = scene
        .represent(
            volume,
            Representation::volume()
                .isolevel(2.0)
                .volume_style(VolumeStyle {
                    rendering: VolumeRendering::IsoMesh,
                    iso_width_voxels: 0.12,
                    ..VolumeStyle::isosurface()
                }),
        )
        .unwrap();
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 10.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 7.0,
            aspect: 1.0,
            near: 0.1,
            far: 30.0,
        },
    };
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            profile: RenderProfile::bare().with_effect(
                molgfx_render::PresentationEffect::Backdrop(BackdropStyle::transparent()),
            ),
            ..EngineConfig::default()
        },
        None,
    )
    .unwrap();
    let config = ImageConfig {
        width: 128,
        height: 128,
    };
    let mesh = engine.render_image(&scene, &camera, config).unwrap();
    scene.representation_mut(handle).unwrap().volume.rendering = VolumeRendering::IsoDots;
    let dots = engine.render_image(&scene, &camera, config).unwrap();
    let mesh_coverage = coverage(&mesh);
    let dot_coverage = coverage(&dots);
    assert!(dot_coverage > 100, "dots must draw: {dot_coverage}");
    assert!(
        mesh_coverage > dot_coverage * 2,
        "lattice lines and circles must differ: {mesh_coverage}, {dot_coverage}"
    );
    scene.representation_mut(handle).unwrap().volume.region =
        Some(molgfx_core::VolumeRegion::new([0, 0, 3], [5; 3], [5; 3]).unwrap());
    let cropped = engine.render_image(&scene, &camera, config).unwrap();
    assert_eq!(coverage(&cropped), 0, "crop excludes the extracted plane");
    scene.representation_mut(handle).unwrap().volume.region = None;
    for rendering in [VolumeRendering::IsoMesh, VolumeRendering::IsoDots] {
        for level in [-1.0, 5.0] {
            let representation = scene.representation_mut(handle).unwrap();
            representation.volume.rendering = rendering;
            representation.params.isolevel = level;
            let empty = engine.render_image(&scene, &camera, config).unwrap();
            assert_eq!(coverage(&empty), 0, "empty boundary: {rendering:?} {level}");
        }
    }
}
