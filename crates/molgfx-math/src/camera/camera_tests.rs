use super::*;
use crate::Vec4;
use proptest::prelude::*;

fn test_camera() -> Camera {
    Camera {
        eye: Vec3::new(3.0, 4.0, 10.0),
        target: Vec3::new(0.0, 1.0, 0.0),
        up: Vec3::Y,
        projection: Projection::Perspective {
            fov_y: 0.9,
            aspect: 1.6,
            near: 0.5,
            far: 200.0,
        },
    }
}

fn clip_visible(view_proj: &Mat4, p: Vec3) -> bool {
    let c = *view_proj * Vec4::new(p.x, p.y, p.z, 1.0);
    if c.w <= 0.0 {
        return false;
    }
    let ndc = c / c.w;
    ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0 && (0.0..=1.0).contains(&ndc.z)
}

fn inside_all_planes(planes: &[Vec4; 6], p: Vec3) -> bool {
    planes
        .iter()
        .all(|plane| plane.truncate().dot(p) + plane.w >= 0.0)
}

#[test]
fn the_view_matrix_places_the_target_on_the_negative_z_axis() {
    let cam = test_camera();
    let view_target = cam.view().transform_point3(cam.target);
    assert!(view_target.x.abs() < 1e-4);
    assert!(view_target.y.abs() < 1e-4);
    assert!(view_target.z < 0.0);
}

#[test]
fn a_framing_camera_sees_the_whole_bounding_sphere() {
    let bound = BoundingSphere {
        center: Vec3::new(10.0, -5.0, 3.0),
        radius: 25.0,
    };
    let cam = Camera::framing(&bound, 1.5);
    let vp = cam.view_proj();
    // Sample the sphere surface; every sample must land in clip bounds.
    for i in 0u8..40 {
        let a = f32::from(i) * 0.157;
        let dir = Vec3::new(a.cos() * (a * 0.7).sin(), a.sin(), (a * 0.7).cos());
        let p = bound.center + dir.normalize() * bound.radius * 0.99;
        assert!(clip_visible(&vp, p), "sphere surface point left the frame");
    }
}

#[test]
fn aabb_framing_is_tight_and_keeps_every_corner_visible() {
    let bound = Aabb::new(Vec3::new(-20.0, -8.0, -4.0), Vec3::new(20.0, 8.0, 4.0));
    let camera = Camera::framing_aabb(&bound, 16.0 / 9.0);
    for corner in bound.corners() {
        assert!(clip_visible(&camera.view_proj(), corner));
    }
    assert!(camera.focus_distance() < bound.bounding_sphere().radius * 3.0);
}

proptest! {
    #[test]
    fn frustum_planes_agree_with_the_clip_space_test(
        x in -60.0f32..60.0, y in -60.0f32..60.0, z in -60.0f32..60.0,
    ) {
        let cam = test_camera();
        let p = Vec3::new(x, y, z);
        let by_clip = clip_visible(&cam.view_proj(), p);
        let by_planes = inside_all_planes(&cam.frustum_planes(), p);
        // Skip razor-edge disagreements from float rounding at the boundary.
        let planes = cam.frustum_planes();
        let min_margin = planes
            .iter()
            .map(|pl| pl.truncate().dot(p) + pl.w)
            .fold(f32::INFINITY, f32::min);
        prop_assume!(min_margin.abs() > 1e-3);
        prop_assert_eq!(by_clip, by_planes);
    }
}
