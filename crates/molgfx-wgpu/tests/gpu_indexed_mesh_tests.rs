//! Native indexed meshes preserve coverage, transparency and picking.
#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{EntityKind, Material, Mesh, MeshVertex, Scene};
use molgfx_math::{Camera, Projection, Rgba8, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PickEntity, PresentationEffect, RenderMode,
    RenderProfile,
};
use molgfx_wgpu::WgpuDevice;

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_nonsequential_index_stream_preserves_visible_mesh_identity_and_alpha() {
    let (source, _) = molframe::read_bytes(
        b"ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n"
            .to_vec(),
        Some("mesh-owner.pdb"),
        &molframe::ReadOptions::new(),
    )
    .unwrap();
    let mut scene = Scene::from_structure(&source).unwrap();
    let owner = scene.structures().next().unwrap().0;
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 10.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 6.0,
            aspect: 1.0,
            near: 0.1,
            far: 30.0,
        },
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
    .unwrap();
    let mut images = Vec::new();
    for alpha in [255, 96] {
        let vertices = [
            Vec3::new(100.0, 100.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(-2.0, -2.0, 0.0),
            Vec3::new(2.0, -2.0, 0.0),
        ]
        .map(|position| MeshVertex {
            position,
            normal: Vec3::Z,
            color: Rgba8::new(80, 160, 240, alpha),
        });
        let mesh = Mesh::new(owner, vertices.to_vec(), vec![2, 3, 1], Material::default()).unwrap();
        let handle = scene.add_mesh(mesh).unwrap();
        let image = engine
            .render_image(
                &scene,
                &camera,
                ImageConfig {
                    width: 96,
                    height: 96,
                },
            )
            .unwrap();
        assert!(
            image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[3] > 0)
                .count()
                > 1_000
        );
        assert_eq!(
            image.pixels[(48 * 96 + 10) * 4 + 3],
            0,
            "unused vertex draws no triangle"
        );
        if alpha == 255 {
            let picked = engine
                .pick(48, 48)
                .unwrap()
                .expect("opaque mesh is pickable");
            assert!(
                matches!(picked.entity, PickEntity::Structure(identity) if identity.kind() == EntityKind::Mesh)
            );
        }
        images.push(image);
        scene.remove_mesh(handle);
    }
    let center = (48 * 96 + 48) * 4 + 3;
    assert_eq!(images[0].pixels[center], 255);
    assert!(images[1].pixels[center] > 0 && images[1].pixels[center] < 255);
}
