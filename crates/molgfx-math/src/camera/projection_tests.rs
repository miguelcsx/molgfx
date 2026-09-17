use super::*;
use crate::Vec4;

fn project_depth(projection: &Projection, view_z: f32) -> f32 {
    // View space looks down −Z, so a point at distance d sits at z = −d.
    let clip = projection.matrix() * Vec4::new(0.0, 0.0, -view_z, 1.0);
    clip.z / clip.w
}

#[test]
fn perspective_depth_is_one_at_the_near_plane_and_zero_at_the_far_plane() {
    let p = Projection::Perspective {
        fov_y: 1.0,
        aspect: 1.5,
        near: 2.0,
        far: 100.0,
    };
    assert!((project_depth(&p, 2.0) - 1.0).abs() < 1e-5);
    assert!(project_depth(&p, 100.0).abs() < 1e-5);
}

#[test]
fn orthographic_depth_is_one_at_the_near_plane_and_zero_at_the_far_plane() {
    let p = Projection::Orthographic {
        height: 50.0,
        aspect: 1.0,
        near: 1.0,
        far: 200.0,
    };
    assert!((project_depth(&p, 1.0) - 1.0).abs() < 1e-5);
    assert!(project_depth(&p, 200.0).abs() < 1e-5);
}

#[test]
fn depth_decreases_monotonically_with_distance_between_the_planes() {
    let p = Projection::Perspective {
        fov_y: 0.8,
        aspect: 1.0,
        near: 1.0,
        far: 500.0,
    };
    let mut previous = f32::INFINITY;
    for step in 0u8..50 {
        let d = 1.0 + f32::from(step) * 10.0;
        let depth = project_depth(&p, d);
        assert!(depth < previous, "depth must fall as distance grows");
        previous = depth;
    }
}

#[test]
fn fitting_near_and_far_brackets_the_bounding_sphere() {
    let mut p = Projection::Perspective {
        fov_y: 1.0,
        aspect: 1.0,
        near: 0.1,
        far: 10.0,
    };
    let bound = BoundingSphere {
        center: Vec3::new(0.0, 0.0, -50.0),
        radius: 10.0,
    };
    p.fit_near_far(Vec3::ZERO, &bound);
    let Projection::Perspective { near, far, .. } = p else {
        panic!("projection kind must be preserved");
    };
    assert!(near < 40.0 && near > 30.0);
    assert!(far > 60.0 && far < 70.0);
}

#[test]
fn fitting_from_inside_the_sphere_clamps_the_near_plane_positive() {
    let mut p = Projection::Orthographic {
        height: 10.0,
        aspect: 1.0,
        near: 1.0,
        far: 2.0,
    };
    let bound = BoundingSphere {
        center: Vec3::ZERO,
        radius: 100.0,
    };
    p.fit_near_far(Vec3::ZERO, &bound);
    let Projection::Orthographic { near, far, .. } = p else {
        panic!("projection kind must be preserved");
    };
    assert!(near > 0.0);
    assert!(far > near);
}
