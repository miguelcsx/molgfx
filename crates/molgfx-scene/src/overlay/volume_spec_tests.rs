use super::*;
use crate::{Scene, VolumeBinding, density};
use std::sync::Arc;

#[test]
fn a_skewed_voxel_maps_to_the_world_point_the_sampler_reads() {
    let affine = cell_affine([4.0, -2.0, 3.0], [1.0, 2.0, 3.0], [90.0, 90.0, 60.0]).unwrap();
    let volume = VolumeBinding::new(DataSource::new("skew"), [2; 3], Arc::from([0.0; 8]))
        .affine(affine)
        .native()
        .unwrap();
    let voxel = Vec3::new(0.25, 0.5, 0.75);
    let world = volume.voxel_to_world().transform_point3(voxel);
    assert!((world - Vec3::new(4.75, -1.133_974_6, 5.25)).length() < 1.0e-5);
    assert!((volume.voxel_to_world().inverse().transform_point3(world) - voxel).length() < 1.0e-5);
}

#[test]
fn two_isolevels_share_one_volume_upload() {
    let mut scene = Scene::empty();
    let source = DataSource::new("shared");
    scene
        .add(
            density::volume(source.clone(), [2; 3])
                .isosurface(0.25, Color::rgb(255, 0, 0), 1.0)
                .isosurface(0.75, Color::rgb(0, 0, 255), 0.5),
        )
        .unwrap();
    scene
        .bind_volume(VolumeBinding::new(
            source,
            [2; 3],
            Arc::from([0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]),
        ))
        .unwrap();
    assert_eq!(scene.overlay_handles().volumes, 1);
    assert_eq!(scene.resolved().representations().count(), 2);
}

#[test]
fn volume_descriptors_reject_non_affine_matrices_and_invalid_regions() {
    let mut spec = density::volume(DataSource::new("grid"), [2; 3])
        .isosurface(0.5, Color::rgb(1, 2, 3), 1.0)
        .0;
    assert!(spec.validate().is_ok());
    spec.voxel_to_world[3] = 0.5;
    assert!(spec.validate().is_err());
    spec.voxel_to_world = Mat4::IDENTITY.to_cols_array();
    spec.region = Some(VolumeRegion {
        minimum: [0; 3],
        maximum: [3; 3],
    });
    assert!(spec.validate().is_err());
}

#[test]
fn an_isovalue_without_a_crossing_draws_nothing_and_errors_nothing() {
    let mut scene = Scene::empty();
    let source = DataSource::new("empty-contour");
    scene
        .add(density::volume(source.clone(), [2; 3]).isosurface(10.0, Color::rgb(1, 2, 3), 1.0))
        .unwrap();
    scene
        .bind_volume(VolumeBinding::new(source, [2; 3], Arc::from([0.0; 8])))
        .unwrap();
    let (_, rep) = scene.resolved().representations().next().unwrap();
    assert!((rep.params.isolevel - 10.0).abs() < f32::EPSILON);
    assert!(scene.unresolved_overlays().is_empty());
}

#[test]
fn volume_descriptors_reject_unrepresentable_steps_and_inverses_before_binding() {
    let mut spec = density::volume(DataSource::new("grid"), [2; 3])
        .direct(
            vec![
                crate::VolumeTransferPoint {
                    value: 0.0,
                    color: Color::rgb(0, 0, 0),
                    opacity: 0.0,
                },
                crate::VolumeTransferPoint {
                    value: 1.0,
                    color: Color::rgb(255, 255, 255),
                    opacity: 1.0,
                },
            ],
            1.0,
            f32::MIN_POSITIVE,
        )
        .0;
    spec.voxel_to_world = Mat4::from_scale_rotation_translation(
        molgfx_math::Vec3::splat(0.001),
        molgfx_math::Quat::default(),
        molgfx_math::Vec3::ZERO,
    )
    .to_cols_array();
    assert!(spec.validate().is_err());
    spec.voxel_to_world = Mat4::from_scale_rotation_translation(
        molgfx_math::Vec3::new(1.0e-30, 1.0e20, 1.0e20),
        molgfx_math::Quat::default(),
        molgfx_math::Vec3::ZERO,
    )
    .to_cols_array();
    spec.voxel_to_world[12] = 1.0e20;
    assert!(spec.validate().is_err());
}
