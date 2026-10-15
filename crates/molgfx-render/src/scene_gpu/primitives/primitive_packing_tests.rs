use super::*;
use molgfx_core::{ParticleBoundary, ParticleMotion};
use molgfx_math::Aabb;

#[test]
fn particle_motion_packs_the_three_inverse_boundary_spans() {
    let motion = match ParticleMotion::new(
        Vec3::new(2.0, 4.0, 8.0),
        Aabb::new(Vec3::new(-1.0, -2.0, -4.0), Vec3::new(3.0, 6.0, 12.0)),
        0.5,
        17,
        ParticleBoundary::Wrap,
    ) {
        Ok(value) => value,
        Err(error) => panic!("particle motion fixture is valid: {error}"),
    };

    let packed = pack_motion(motion, Mat4::IDENTITY);

    assert_eq!(
        packed.velocity_step.map(f32::to_bits),
        [1.0, 2.0, 4.0, 0.25].map(f32::to_bits)
    );
    assert_eq!(
        packed.minimum.map(f32::to_bits),
        [-1.0, -2.0, -4.0, 0.125].map(f32::to_bits)
    );
    assert_eq!(
        packed.maximum.map(f32::to_bits),
        [3.0, 6.0, 12.0, 0.0625].map(f32::to_bits)
    );
}
