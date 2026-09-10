use super::*;

#[test]
fn template_precomputes_one_instance_per_atom_and_bond() {
    let template = LicoriceTemplate::new(Vec3::ZERO, vec![Vec3::ZERO, Vec3::Z], &[[0, 1]]);

    let template = match template {
        Ok(value) => value,
        Err(error) => panic!("template validates: {error}"),
    };
    assert_eq!(template.instances_per_pose(), 3);
}

#[test]
fn template_rejects_stale_bond_rows() {
    assert!(matches!(
        LicoriceTemplate::new(Vec3::ZERO, vec![Vec3::ZERO], &[[0, 1]]),
        Err(CoreError::InvalidPrimitive { .. })
    ));
}

#[test]
fn pose_rejects_a_quaternion_whose_norm_overflows() {
    let pose = LigandPose::new(
        Vec3::ZERO,
        Quat::from_xyzw(f32::MAX, f32::MAX, f32::MAX, f32::MAX),
        Rgba8::WHITE,
        1.0,
    );

    assert!(matches!(pose, Err(CoreError::InvalidPrimitive { .. })));
}
