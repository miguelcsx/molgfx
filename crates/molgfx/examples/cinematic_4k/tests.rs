use super::*;
use molgfx::Aabb;

#[test]
fn the_initial_orbit_preserves_the_framing_cameras_up_axis() {
    let bounds = Aabb::new(Vec3::new(-20.0, -4.0, -10.0), Vec3::new(20.0, 4.0, 10.0));
    let sphere = bounds.bounding_sphere();
    let base = Camera::framing_aabb(&bounds, ASPECT);
    let camera = orbit_camera(base, base.eye - base.target, &sphere, 0.0);

    assert!(camera.view().is_finite());
}
