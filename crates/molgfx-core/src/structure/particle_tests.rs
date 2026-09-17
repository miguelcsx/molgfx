use super::*;

#[test]
fn particle_motion_rejects_non_finite_or_degenerate_bounds() {
    assert!(
        ParticleMotion::new(
            Vec3::X,
            Aabb::new(Vec3::ONE, Vec3::ZERO),
            0.016,
            4,
            ParticleBoundary::Bounce,
        )
        .is_err()
    );
    assert!(
        ParticleMotion::new(
            Vec3::NAN,
            Aabb::new(Vec3::splat(-1.0), Vec3::splat(1.0)),
            0.016,
            4,
            ParticleBoundary::Wrap,
        )
        .is_err()
    );
}

#[test]
fn particle_motion_retains_fixed_step_boundary_and_seed() {
    let motion = match ParticleMotion::new(
        Vec3::new(1.0, -2.0, 0.5),
        Aabb::new(Vec3::splat(-3.0), Vec3::splat(3.0)),
        0.125,
        17,
        ParticleBoundary::Wrap,
    ) {
        Ok(motion) => motion,
        Err(error) => panic!("particle motion validates: {error}"),
    };
    assert!((motion.fixed_timestep() - 0.125).abs() < f32::EPSILON);
    assert_eq!(motion.seed(), 17);
    assert_eq!(motion.boundary(), ParticleBoundary::Wrap);
    assert_eq!(motion.respawn_after_steps(), 0);
    assert_eq!(
        motion.with_respawn_after_steps(12).respawn_after_steps(),
        12
    );
}

#[test]
fn particle_motion_is_presentation_state_separate_from_source_pose() {
    let motion = match ParticleMotion::new(
        Vec3::X,
        Aabb::new(Vec3::splat(-2.0), Vec3::splat(2.0)),
        0.016,
        1,
        ParticleBoundary::Bounce,
    ) {
        Ok(motion) => motion,
        Err(error) => panic!("particle motion validates: {error}"),
    };
    let particle = match Particle::new(
        StructureHandle(crate::handle::RawHandle::new_for_test(0, 0)),
        Vec3::new(0.5, 0.0, 0.0),
        Quat::IDENTITY,
        Vec3::splat(1.0),
        ParticleShape::Sphere,
        Rgba8::WHITE,
        1.0,
    ) {
        Ok(particle) => particle.with_motion(motion),
        Err(error) => panic!("particle validates: {error}"),
    };
    assert_eq!(particle.center, Vec3::new(0.5, 0.0, 0.0));
    assert_eq!(particle.motion, Some(motion));
}

#[test]
fn superquadric_exponents_are_explicit_and_bounded() {
    let particle = Particle::new(
        StructureHandle(crate::handle::RawHandle::new_for_test(0, 0)),
        Vec3::ZERO,
        Quat::IDENTITY,
        Vec3::splat(2.0),
        ParticleShape::Superquadric,
        Rgba8::WHITE,
        1.0,
    );
    let Ok(particle) = particle else {
        panic!("superquadric validates")
    };
    let shaped = particle.with_superquadric_exponents(0.5, 2.0);
    assert!(
        matches!(shaped, Ok(value) if value.shape_parameters.iter().zip([0.5, 2.0]).all(|(left, right)| (*left - right).abs() < 1.0e-6))
    );
    assert!(particle.with_superquadric_exponents(0.0, 2.0).is_err());
}
