use super::tests::{camera, engine};
use crate::{scene_gpu::GeometryKind, testing::MockLog};
use molgfx_core::{Representation, ScalarVolume, Scene, VolumeRendering, VolumeStyle};
use molgfx_math::Mat4;
use std::sync::Arc;

fn writes(log: &MockLog, label: &str) -> usize {
    let buffers = log.buffers.lock().unwrap();
    log.writes
        .lock()
        .unwrap()
        .iter()
        .filter(|write| {
            buffers
                .iter()
                .any(|buffer| buffer.0 == write.0 && buffer.1 == label)
        })
        .count()
}

#[test]
fn mesh_and_dots_share_geometry_and_style_edits_do_not_rebuild_it() {
    let mut scene = Scene::new();
    let grid = ScalarVolume::new(
        [2; 3],
        Mat4::IDENTITY,
        Arc::from([0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]),
    )
    .unwrap();
    let volume = scene.add_volume(grid);
    let mut handles = Vec::new();
    for rendering in [VolumeRendering::IsoMesh, VolumeRendering::IsoDots] {
        let representation = Representation::volume().volume_style(VolumeStyle {
            rendering,
            ..VolumeStyle::isosurface()
        });
        let representation = representation.isolevel(0.5);
        handles.push(scene.represent(volume, representation).unwrap());
    }
    let mut engine = engine();
    engine.render(&scene, &camera()).unwrap();
    assert_eq!(writes(&engine.device.log, "field boundary vertices"), 1);
    assert_eq!(writes(&engine.device.log, "field boundary indices"), 1);
    assert_eq!(engine.scene_gpu.volume_draws().count(), 2);
    assert!(
        engine
            .scene_gpu
            .volume_draws()
            .all(|(_, _, geometry)| geometry.kind() == GeometryKind::Boundary)
    );
    for handle in &handles {
        let representation = scene.representation_mut(*handle).unwrap();
        representation.volume.iso_width_voxels = 0.2;
        representation.material.opacity = 0.4;
    }
    engine.render(&scene, &camera()).unwrap();
    engine.render(&scene, &camera()).unwrap();
    assert_eq!(writes(&engine.device.log, "field boundary vertices"), 1);
    assert_eq!(writes(&engine.device.log, "field boundary indices"), 1);
    assert_eq!(
        engine
            .device
            .log
            .texture_writes
            .lock()
            .unwrap()
            .iter()
            .filter(|write| write.0 == "caller density volume")
            .count(),
        1
    );
    for handle in &handles {
        scene.representation_mut(*handle).unwrap().params.isolevel = 0.7;
    }
    engine.render(&scene, &camera()).unwrap();
    assert_eq!(writes(&engine.device.log, "field boundary vertices"), 2);
    for handle in &handles {
        scene.representation_mut(*handle).unwrap().params.isolevel = 0.5;
    }
    engine.render(&scene, &camera()).unwrap();
    assert_eq!(
        writes(&engine.device.log, "field boundary vertices"),
        3,
        "inactive levels release geometry instead of accumulating a history cache"
    );
    assert_eq!(
        engine
            .device
            .log
            .texture_writes
            .lock()
            .unwrap()
            .iter()
            .filter(|write| write.0 == "caller density volume")
            .count(),
        1
    );
}

#[test]
fn constant_fields_have_no_mesh_or_dot_drawables() {
    let mut scene = Scene::new();
    let volume =
        scene.add_volume(ScalarVolume::new([2; 3], Mat4::IDENTITY, Arc::from([1.0; 8])).unwrap());
    for rendering in [VolumeRendering::IsoMesh, VolumeRendering::IsoDots] {
        for level in [0.0, 1.0, 2.0] {
            let representation = Representation::volume().volume_style(VolumeStyle {
                rendering,
                ..VolumeStyle::isosurface()
            });
            let representation = representation.isolevel(level);
            scene.represent(volume, representation).unwrap();
        }
    }
    let mut engine = engine();
    engine.render(&scene, &camera()).unwrap();
    assert_eq!(engine.scene_gpu.volume_draws().count(), 0);
}

#[test]
fn malformed_live_scalar_styles_return_errors_instead_of_becoming_valid_defaults() {
    for rendering in [VolumeRendering::Direct, VolumeRendering::IsoMesh] {
        let mut scene = Scene::new();
        let volume = scene.add_volume(
            ScalarVolume::new(
                [2; 3],
                Mat4::IDENTITY,
                Arc::from([0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]),
            )
            .unwrap(),
        );
        let handle = scene
            .represent(
                volume,
                Representation::volume()
                    .isolevel(0.5)
                    .volume_style(VolumeStyle {
                        rendering,
                        ..VolumeStyle::default()
                    }),
            )
            .unwrap();
        let mut engine = engine();
        engine.render(&scene, &camera()).unwrap();
        scene.representation_mut(handle).unwrap().params.isolevel = f32::NAN;
        assert!(engine.render(&scene, &camera()).is_err());
        let representation = scene.representation_mut(handle).unwrap();
        representation.params.isolevel = 0.5;
        representation.volume.step_scale = 0.0;
        assert!(matches!(
            engine.render(&scene, &camera()),
            Err(crate::RenderError::RepresentationInput(
                molgfx_core::CoreError::InvalidVolume { .. }
            ))
        ));
        scene.representation_mut(handle).unwrap().volume.step_scale = 0.65;
        engine.render(&scene, &camera()).unwrap();
    }
}
