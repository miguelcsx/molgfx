use crate::{Camera, Projection, Vec3};

fn camera() -> Camera {
    Camera {
        eye: Vec3::Z * 20.0,
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Perspective {
            fov_y: 0.8,
            aspect: 1.0,
            near: 0.1,
            far: 100.0,
        },
    }
}

#[test]
fn stereo_converges_without_vertical_parallax_and_rays_recover_each_eye_projection() {
    let eyes = camera().stereo_eyes(1.2, 20.0).expect("stereo validates");
    let center = eyes.map(|eye| {
        eye.project(Vec3::ZERO, (256, 256))
            .expect("target projects")
    });
    assert!((center[0].x - center[1].x).abs() < 1e-4);
    for point in [Vec3::new(2.0, 1.0, 5.0), Vec3::new(-1.0, 2.0, -5.0)] {
        let projected = eyes.map(|eye| eye.project(point, (256, 256)).expect("point projects"));
        assert!((projected[0].y - projected[1].y).abs() < 1e-4);
        assert!((projected[0].x - projected[1].x).abs() > 0.1);
        for (eye, pixel) in eyes.iter().zip(projected) {
            let ray = eye
                .ray(pixel.x, pixel.y, (256, 256))
                .expect("ray constructs");
            assert!(
                (point - ray.origin)
                    .normalize()
                    .cross(ray.direction)
                    .length()
                    < 1e-5
            );
        }
    }
    assert!(camera().stereo_eyes(-1.0, 20.0).is_none());
    assert!(camera().stereo_eyes(1.0, 0.0).is_none());
}

#[test]
fn invalid_projections_are_rejected_even_when_the_matrix_is_finite() {
    let mut invalid = camera();
    invalid.projection = Projection::Perspective {
        fov_y: -0.8,
        aspect: 1.0,
        near: 0.1,
        far: 100.0,
    };
    assert!(invalid.stereo_eyes(1.0, 20.0).is_none());
    invalid.projection = Projection::Orthographic {
        height: 12.0,
        aspect: 1.0,
        near: 0.1,
        far: 100.0,
    };
    let [left, right] = invalid
        .stereo_eyes(1.0, 20.0)
        .expect("parallel orthographic cameras");
    assert_eq!(left.eye, right.eye);
    assert_eq!(left.projection.matrix(), right.projection.matrix());
}
