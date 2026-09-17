use super::*;

#[test]
fn columnar_pose_marshalling_preserves_every_typed_row() {
    let translations = [1.0_f32, 2.0, 3.0, -4.0, -5.0, -6.0];
    let orientations = [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0];
    let colors = [1_u8, 2, 3, 4, 101, 102, 103, 104];
    let opacities = [0.25_f32, 0.75];
    let mut poses = Vec::new();
    if let Err(error) = poses.try_reserve_exact(2) {
        panic!("two-pose scratch reserves: {error}")
    }

    if let Err(error) = extend_pose_columns(
        &mut poses,
        &translations,
        &orientations,
        &colors,
        Some(&opacities),
    ) {
        panic!("valid pose columns marshal: {error}")
    }

    assert_eq!(poses.len(), 2);
    assert!(poses.capacity() >= poses.len());
    assert_eq!(poses[0].translation(), molgfx::Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(poses[0].color(), molgfx::Rgba8::new(1, 2, 3, 4));
    assert_eq!(poses[0].opacity().to_bits(), 0.25_f32.to_bits());
    assert_eq!(poses[1].translation(), molgfx::Vec3::new(-4.0, -5.0, -6.0));
    assert_eq!(poses[1].color(), molgfx::Rgba8::new(101, 102, 103, 104));
    assert_eq!(poses[1].opacity().to_bits(), 0.75_f32.to_bits());
}

#[test]
fn columnar_pose_marshalling_uses_default_opacity_without_an_extra_column() {
    let mut poses = Vec::new();
    if let Err(error) = poses.try_reserve_exact(1) {
        panic!("one-pose scratch reserves: {error}")
    }
    if let Err(error) = extend_pose_columns(
        &mut poses,
        &[0.0, 0.0, 0.0],
        &[0.0, 0.0, 0.0, 1.0],
        &[20, 40, 60, 255],
        None,
    ) {
        panic!("default-opacity pose marshals: {error}")
    }

    assert_eq!(poses.len(), 1);
    assert_eq!(poses[0].opacity().to_bits(), 1.0_f32.to_bits());
}

#[test]
fn malformed_quaternions_are_rejected_before_scene_mutation() {
    let mut poses = Vec::new();
    if let Err(error) = poses.try_reserve_exact(1) {
        panic!("one-pose scratch reserves: {error}")
    }

    assert!(
        extend_pose_columns(
            &mut poses,
            &[0.0, 0.0, 0.0],
            &[0.0, 0.0, 0.0, 0.0],
            &[255; 4],
            None,
        )
        .is_err()
    );
    assert!(poses.is_empty());
}

#[test]
fn columnar_pose_shapes_require_the_declared_width() {
    assert!(validate_matrix("translations", &[2, 3], 2, 3).is_ok());
    assert!(validate_matrix("translations", &[2, 4], 2, 3).is_err());
    assert!(validate_matrix("translations", &[6], 2, 3).is_err());
}
