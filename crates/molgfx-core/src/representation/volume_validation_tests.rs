use crate::{CoreError, Representation, ScalarVolume, Scene, VolumeStyle};
use molgfx_math::Mat4;
use std::sync::Arc;

#[test]
fn malformed_volume_sampling_fails_before_a_representation_is_added() {
    let volume = ScalarVolume::new([2; 3], Mat4::IDENTITY, Arc::from([0.5; 8])).unwrap();
    for (opacity, step) in [
        (1.0, 0.0),
        (1.0, -1.0),
        (1.0, f32::NAN),
        (1.0, f32::INFINITY),
        (-1.0, 0.65),
        (f32::NAN, 0.65),
        (f32::INFINITY, 0.65),
    ] {
        let mut scene = Scene::new();
        let handle = scene.add_volume(volume.clone());
        let result = scene.represent(
            handle,
            Representation::volume().volume_style(VolumeStyle::default().sampling(opacity, step)),
        );
        assert!(matches!(result, Err(CoreError::InvalidVolume { .. })));
        assert_eq!(scene.representation_count(), 0);
    }
}

#[test]
fn positive_sampling_scales_remain_valid_outside_previous_renderer_clamps() {
    let volume = ScalarVolume::new([2; 3], Mat4::IDENTITY, Arc::from([0.5; 8])).unwrap();
    for step in [0.05_f32, 4.0] {
        let mut scene = Scene::new();
        let handle = scene.add_volume(volume.clone());
        assert!(
            scene
                .represent(
                    handle,
                    Representation::volume()
                        .volume_style(VolumeStyle::default().sampling(1.0, step))
                )
                .is_ok()
        );
    }
}

#[test]
fn a_crop_validated_for_a_larger_grid_is_rejected_on_a_smaller_source() {
    let volume = ScalarVolume::new([2; 3], Mat4::IDENTITY, Arc::from([0.5; 8])).unwrap();
    let mut scene = Scene::new();
    let handle = scene.add_volume(volume);
    let crop = crate::VolumeRegion::new([0; 3], [3; 3], [3; 3]).unwrap();
    let result = scene.represent(
        handle,
        Representation::volume().volume_style(VolumeStyle::default().region(crop)),
    );
    assert!(matches!(result, Err(CoreError::InvalidVolume { .. })));
    assert_eq!(scene.representation_count(), 0);
}

#[test]
fn an_unrepresentable_world_ray_step_returns_a_typed_error() {
    let volume = ScalarVolume::new([2; 3], Mat4::IDENTITY, Arc::from([0.5; 8])).unwrap();
    let mut scene = Scene::new();
    let handle = scene.add_volume(volume);
    let result = scene.represent(
        handle,
        Representation::volume()
            .volume_style(VolumeStyle::default().sampling(1.0, f32::from_bits(1))),
    );
    assert!(matches!(result, Err(CoreError::InvalidVolume { .. })));
    assert_eq!(scene.representation_count(), 0);
}

#[test]
fn a_transform_with_an_unrepresentable_inverse_is_rejected_before_upload() {
    let transform = Mat4::from_scale_rotation_translation(
        molgfx_math::Vec3::new(1.0e20, 1.0e-39, 1.0e20),
        molgfx_math::Quat::IDENTITY,
        molgfx_math::Vec3::ZERO,
    );
    assert!(matches!(
        ScalarVolume::new([2; 3], transform, Arc::from([0.5; 8])),
        Err(CoreError::InvalidVolume { .. })
    ));
}

#[test]
fn nonfinite_isolevels_fail_before_a_representation_is_added() {
    let volume = ScalarVolume::new([2; 3], Mat4::IDENTITY, Arc::from([0.5; 8])).unwrap();
    for level in [f32::NAN, f32::NEG_INFINITY, f32::INFINITY] {
        let mut scene = Scene::new();
        let handle = scene.add_volume(volume.clone());
        let result = scene.represent(handle, Representation::volume().isolevel(level));
        assert!(matches!(result, Err(CoreError::InvalidVolume { .. })));
        assert_eq!(scene.representation_count(), 0);
    }
}

#[test]
fn invalid_lattice_widths_fail_before_mesh_or_dots_are_added() {
    let volume = ScalarVolume::new([2; 3], Mat4::IDENTITY, Arc::from([0.5; 8])).unwrap();
    for rendering in [
        crate::VolumeRendering::IsoMesh,
        crate::VolumeRendering::IsoDots,
    ] {
        for width in [0.0, -0.1, 0.6, f32::NAN, f32::INFINITY] {
            let mut scene = Scene::new();
            let handle = scene.add_volume(volume.clone());
            let result = scene.represent(
                handle,
                Representation::volume().volume_style(VolumeStyle {
                    rendering,
                    iso_width_voxels: width,
                    ..VolumeStyle::isosurface()
                }),
            );
            assert!(matches!(result, Err(CoreError::InvalidVolume { .. })));
            assert_eq!(scene.representation_count(), 0);
        }
    }
}
