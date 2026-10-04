use super::Mat4;
use crate::{Quat, Vec3};

#[test]
fn anisotropic_axis_lengths_do_not_underflow_when_squared() {
    let transform = Mat4::from_scale_rotation_translation(
        Vec3::new(1.0e20, 1.0e-30, 1.0e20),
        Quat::IDENTITY,
        Vec3::ZERO,
    );
    assert_eq!(
        transform.minimum_axis_length().to_bits(),
        1.0e-30_f32.to_bits()
    );
}

#[test]
fn axis_lengths_include_shear_and_ignore_translation() {
    let mut transform = Mat4::IDENTITY;
    transform.x_axis.y = 3.0;
    transform.x_axis.x = 4.0;
    transform.y_axis.y = 6.0;
    transform.z_axis.z = 7.0;
    transform.w_axis.x = 1000.0;
    assert_eq!(transform.minimum_axis_length().to_bits(), 5.0_f32.to_bits());
}
