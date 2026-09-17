use super::*;
use proptest::prelude::*;

fn finite_vec3() -> impl Strategy<Value = Vec3> {
    let coord = -1000.0f32..1000.0f32;
    (coord.clone(), coord.clone(), coord).prop_map(|(x, y, z)| Vec3::new(x, y, z))
}

#[test]
fn an_empty_box_contains_nothing_and_unions_as_identity() {
    let empty = Aabb::EMPTY;
    assert!(empty.is_empty());
    let other = Aabb::new(Vec3::splat(-1.0), Vec3::splat(2.0));
    assert_eq!(empty.union(&other), other);
    assert_eq!(other.union(&empty), other);
}

#[test]
fn extending_with_a_non_finite_point_leaves_the_box_unchanged() {
    let mut aabb = Aabb::from_points([Vec3::ZERO, Vec3::ONE]);
    let before = aabb;
    aabb.extend(Vec3::new(f32::NAN, 0.0, 0.0));
    aabb.extend(Vec3::new(0.0, f32::INFINITY, 0.0));
    assert_eq!(aabb, before);
}

#[test]
fn a_ray_through_the_center_hits_and_a_parallel_offset_ray_misses() {
    let aabb = Aabb::new(Vec3::splat(-1.0), Vec3::splat(1.0));
    let origin = Vec3::new(0.0, 0.0, 5.0);
    let dir = Vec3::new(0.0, 0.0, -1.0);
    let hit = aabb.ray_intersect(origin, dir.recip());
    let Some((t_near, t_far)) = hit else {
        panic!("ray through the center must hit")
    };
    assert!(t_near < t_far);
    assert!((t_near - 4.0).abs() < 1e-5);

    let missed = aabb.ray_intersect(Vec3::new(5.0, 0.0, 5.0), dir.recip());
    assert!(missed.is_none());
}

#[test]
fn a_ray_starting_inside_the_box_reports_a_zero_entry_distance() {
    let aabb = Aabb::new(Vec3::splat(-1.0), Vec3::splat(1.0));
    let hit = aabb.ray_intersect(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0).recip());
    let Some((t_near, _)) = hit else {
        panic!("interior ray must hit")
    };
    assert!(t_near.abs() < f32::EPSILON);
}

proptest! {
    #[test]
    fn every_source_point_lies_inside_the_built_box(points in prop::collection::vec(finite_vec3(), 1..64)) {
        let aabb = Aabb::from_points(points.clone());
        for p in points {
            prop_assert!(aabb.min.x <= p.x && p.x <= aabb.max.x);
            prop_assert!(aabb.min.y <= p.y && p.y <= aabb.max.y);
            prop_assert!(aabb.min.z <= p.z && p.z <= aabb.max.z);
        }
    }

    #[test]
    fn a_transformed_box_bounds_all_transformed_source_points(
        points in prop::collection::vec(finite_vec3(), 1..32),
        translation in finite_vec3(),
        angle in 0.0f32..std::f32::consts::TAU,
    ) {
        let matrix = Mat4::from_translation(translation)
            * Mat4::from_rotation_y(angle);
        let transformed_box = Aabb::from_points(points.clone()).transform(&matrix);
        for p in points {
            let q = matrix.transform_point3(p);
            let eps = 1e-2 * (1.0 + q.length());
            prop_assert!(transformed_box.min.x <= q.x + eps);
            prop_assert!(transformed_box.max.x >= q.x - eps);
            prop_assert!(transformed_box.min.y <= q.y + eps);
            prop_assert!(transformed_box.max.y >= q.y - eps);
            prop_assert!(transformed_box.min.z <= q.z + eps);
            prop_assert!(transformed_box.max.z >= q.z - eps);
        }
    }

    #[test]
    fn the_bounding_sphere_from_points_contains_every_point(
        points in prop::collection::vec(finite_vec3(), 1..64),
    ) {
        let sphere = BoundingSphere::from_points(&points);
        for p in points {
            let eps = 1e-3 * (1.0 + sphere.radius);
            prop_assert!(sphere.center.distance(p) <= sphere.radius + eps);
        }
    }
}

use crate::Mat4;
