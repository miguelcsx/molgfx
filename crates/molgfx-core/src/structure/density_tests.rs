use super::ScalarVolume;
use molgfx_math::{Mat4, Vec3};
use std::sync::Arc;

#[test]
fn a_valid_volume_keeps_the_callers_shared_values_and_computes_its_range() {
    let values: Arc<[f32]> = Arc::from([0.0, 1.0, -2.0, 3.0, 4.0, 2.0, 1.0, 0.0]);
    let pointer = values.as_ptr();
    let Ok(volume) = ScalarVolume::from_spacing(
        [2, 2, 2],
        Vec3::new(1.0, 2.0, 3.0),
        Vec3::splat(0.5),
        Arc::clone(&values),
    ) else {
        panic!("valid density volume builds")
    };
    assert_eq!(volume.values().as_ptr(), pointer);
    let range = volume.range();
    assert!((range[0] + 2.0).abs() < f32::EPSILON);
    assert!((range[1] - 4.0).abs() < f32::EPSILON);
    assert_eq!(volume.empty_space_dimensions(), [1, 1, 1]);
    assert_eq!(volume.empty_space_bounds(), &[-2.0, 4.0]);
    assert_eq!(volume.world_aabb().min, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(volume.world_aabb().max, Vec3::new(1.5, 2.5, 3.5));
}

#[test]
fn dimensions_values_and_transform_are_validated_before_storage() {
    let values: Arc<[f32]> = Arc::from([0.0; 8]);
    assert!(ScalarVolume::new([1, 2, 4], Mat4::IDENTITY, Arc::clone(&values)).is_err());
    assert!(ScalarVolume::new([2, 2, 3], Mat4::IDENTITY, Arc::clone(&values)).is_err());
    assert!(ScalarVolume::new([2, 2, 2], Mat4::ZERO, values).is_err());
}

#[test]
fn non_finite_density_is_a_typed_error() {
    let values: Arc<[f32]> = Arc::from([0.0, 1.0, f32::NAN, 3.0, 4.0, 2.0, 1.0, 0.0]);
    let Err(error) = ScalarVolume::new([2, 2, 2], Mat4::IDENTITY, values) else {
        panic!("non-finite grid is rejected")
    };
    assert_eq!(error.code(), "MOLGFX-E0032");
}

#[test]
fn empty_space_bounds_include_the_positive_trilinear_halo() {
    let dimensions = [9, 2, 2];
    let mut values = vec![0.0; 36];
    values[8] = 7.0;
    let Ok(volume) = ScalarVolume::new(dimensions, Mat4::IDENTITY, Arc::from(values)) else {
        panic!("valid halo test volume builds")
    };

    assert_eq!(volume.empty_space_dimensions(), [2, 1, 1]);
    assert_eq!(volume.empty_space_bounds(), &[0.0, 7.0, 0.0, 7.0]);
}
