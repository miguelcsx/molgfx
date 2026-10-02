use super::*;
use crate::Projection;

fn perspective() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, 10.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Perspective {
            fov_y: std::f32::consts::FRAC_PI_2,
            aspect: 1.0,
            near: 0.1,
            far: 100.0,
        },
    }
}

#[test]
fn the_target_lands_at_the_centre_of_the_image() {
    let point = perspective().project(Vec3::ZERO, (200, 100));
    let Some(point) = point else {
        panic!("a point in front of the camera projects")
    };
    assert!((point.x - 100.0).abs() < 1e-3, "{point:?}");
    assert!((point.y - 50.0).abs() < 1e-3, "{point:?}");
    assert!((point.depth - 10.0).abs() < 1e-4);
}

#[test]
fn up_in_the_world_is_up_on_the_screen_and_right_is_right() {
    let camera = perspective();
    let (Some(up), Some(right), Some(centre)) = (
        camera.project(Vec3::new(0.0, 2.0, 0.0), (100, 100)),
        camera.project(Vec3::new(2.0, 0.0, 0.0), (100, 100)),
        camera.project(Vec3::ZERO, (100, 100)),
    ) else {
        panic!("all three project")
    };
    assert!(up.y < centre.y, "pixel rows grow downward: {up:?}");
    assert!((up.x - centre.x).abs() < 1e-3);
    assert!(right.x > centre.x);
    assert!((right.y - centre.y).abs() < 1e-3);
}

#[test]
fn a_point_behind_the_eye_or_not_finite_does_not_project() {
    let camera = perspective();
    assert!(
        camera
            .project(Vec3::new(0.0, 0.0, 20.0), (100, 100))
            .is_none()
    );
    assert!(camera.project(Vec3::splat(f32::NAN), (100, 100)).is_none());
}

#[test]
fn the_aspect_follows_the_target_not_the_camera() {
    let camera = perspective();
    let (Some(wide), Some(square)) = (
        camera.project(Vec3::new(5.0, 0.0, 0.0), (200, 100)),
        camera.project(Vec3::new(5.0, 0.0, 0.0), (100, 100)),
    ) else {
        panic!("both project")
    };
    // On a target twice as wide the same world offset covers half the width
    // fraction, so it sits at the same pixel offset from the centre.
    assert!(
        ((wide.x - 100.0) - (square.x - 50.0)).abs() < 1e-2,
        "{wide:?} {square:?}"
    );
}

#[test]
fn a_point_along_a_pixels_ray_projects_back_to_that_pixel() {
    let camera = perspective();
    for (x, y) in [(10.0, 20.0), (150.0, 80.0), (100.0, 50.0)] {
        let Some(ray) = camera.ray(x, y, (200, 100)) else {
            panic!("the ray exists")
        };
        let point = ray.origin + ray.direction * 7.5;
        let Some(back) = camera.project(point, (200, 100)) else {
            panic!("a point on the ray projects")
        };
        assert!(
            (back.x - x).abs() < 1e-2 && (back.y - y).abs() < 1e-2,
            "{back:?}"
        );
    }
}

#[test]
fn an_orthographic_ray_keeps_the_view_direction_and_moves_its_origin() {
    let camera = Camera {
        projection: Projection::Orthographic {
            height: 10.0,
            aspect: 1.0,
            near: 0.1,
            far: 100.0,
        },
        ..perspective()
    };
    let (Some(left), Some(right)) = (
        camera.ray(0.0, 50.0, (100, 100)),
        camera.ray(100.0, 50.0, (100, 100)),
    ) else {
        panic!("both rays exist")
    };
    assert!((left.direction - right.direction).length() < 1e-6);
    assert!((right.origin.x - left.origin.x - 10.0).abs() < 1e-4);
}

#[test]
fn a_degenerate_camera_has_no_ray() {
    let camera = Camera {
        target: perspective().eye,
        ..perspective()
    };
    assert!(camera.ray(0.0, 0.0, (10, 10)).is_none());
}
