use std::mem::{align_of, size_of};

use super::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};

#[test]
fn owned_types_preserve_glam_layout() {
    assert_layout::<Vec2, glam::Vec2>();
    assert_layout::<Vec3, glam::Vec3>();
    assert_layout::<Vec4, glam::Vec4>();
    assert_layout::<Quat, glam::Quat>();
    assert_layout::<Mat3, glam::Mat3>();
    assert_layout::<Mat4, glam::Mat4>();
}

#[test]
fn explicit_glam_round_trips_preserve_bits() {
    let vector = Vec3::new(-1.25, 2.5, 9.75);
    let quaternion = Quat::from_rotation_y(0.75);
    let matrix = Mat4::from_scale_rotation_translation(Vec3::splat(2.0), quaternion, vector);

    assert_eq!(Vec3::from(glam::Vec3::from(vector)), vector);
    assert_eq!(Quat::from(glam::Quat::from(quaternion)), quaternion);
    assert_eq!(Mat4::from(glam::Mat4::from(matrix)), matrix);
}

fn assert_layout<Owned, Internal>() {
    assert_eq!(size_of::<Owned>(), size_of::<Internal>());
    assert_eq!(align_of::<Owned>(), align_of::<Internal>());
}
