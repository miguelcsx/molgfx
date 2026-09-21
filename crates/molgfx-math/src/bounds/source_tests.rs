//! A source has to agree with the materialised boxes it replaces, including in
//! how it treats rows the caller got wrong. Anything else would make the choice
//! between them visible in the picture.

use super::{BvhSource, SphereBounds, SweptSphereBounds};
use crate::{Aabb, Vec3};
use num_traits::ToPrimitive as _;

fn centers(len: usize) -> Vec<[f32; 3]> {
    (0..len)
        .map(|index| {
            let value = index.to_f32().expect("fixture index fits f32");
            [value, value * 0.5 - 2.0, 3.0 - value]
        })
        .collect()
}

#[test]
fn a_slice_source_reports_its_own_bounds_unchanged() {
    let bounds = vec![
        Aabb::new(Vec3::splat(-1.0), Vec3::splat(1.0)),
        Aabb::new(Vec3::ZERO, Vec3::splat(2.0)),
    ];
    let source: &[Aabb] = &bounds;
    assert_eq!(BvhSource::len(&source), 2);
    assert_eq!(source.bound(0), bounds[0]);
    assert_eq!(source.bound(1), bounds[1]);
}

#[test]
fn a_row_past_the_end_is_the_empty_bound_rather_than_a_panic() {
    let bounds = [Aabb::new(Vec3::ZERO, Vec3::ONE)];
    let source: &[Aabb] = &bounds;
    assert_eq!(source.bound(9), Aabb::EMPTY);
    assert_eq!(SphereBounds::new(&[], &[]).bound(0), Aabb::EMPTY);
    assert_eq!(SweptSphereBounds::new(&[], &[], &[]).bound(0), Aabb::EMPTY);
}

#[test]
fn a_sphere_source_matches_the_boxes_it_replaces() {
    let centers = centers(16);
    let radii: Vec<f32> = (0..16)
        .map(|index| 1.0 + index.to_f32().expect("fixture index fits f32") * 0.25)
        .collect();
    let source = SphereBounds::new(&centers, &radii);
    for (index, (center, radius)) in centers.iter().zip(&radii).enumerate() {
        let center = Vec3::from_array(*center);
        let extent = Vec3::splat(*radius);
        let expected = Aabb::new(center - extent, center + extent);
        let row = u32::try_from(index).unwrap_or_else(|_| panic!("test row fits u32"));
        assert_eq!(source.bound(row), expected, "row {index}");
    }
}

#[test]
fn a_sphere_source_stops_at_the_shorter_of_its_two_columns() {
    let centers = centers(8);
    let radii = vec![1.0f32; 3];
    assert_eq!(SphereBounds::new(&centers, &radii).len(), 3);
    assert_eq!(SphereBounds::new(&centers, &radii).bound(4), Aabb::EMPTY);
}

#[test]
fn a_swept_source_encloses_both_endpoints_and_their_radius() {
    let start = [[0.0f32, 0.0, 0.0]];
    let end = [[4.0f32, 0.0, 0.0]];
    let source = SweptSphereBounds::new(&start, &end, &[1.0]);
    assert_eq!(
        source.bound(0),
        Aabb::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(5.0, 1.0, 1.0))
    );
}

#[test]
fn a_negative_radius_still_encloses_its_primitive() {
    let centers = [[0.0f32, 0.0, 0.0]];
    assert_eq!(
        SphereBounds::new(&centers, &[-2.0]).bound(0),
        SphereBounds::new(&centers, &[2.0]).bound(0)
    );
}

#[test]
fn a_hierarchy_over_a_sphere_source_matches_one_over_materialised_boxes() {
    let centers = centers(200);
    let radii: Vec<f32> = (0..200)
        .map(|index| 0.5 + (index % 5).to_f32().expect("fixture index fits f32") * 0.3)
        .collect();
    let source = SphereBounds::new(&centers, &radii);
    let materialised: Vec<Aabb> = (0..200).map(|index| source.bound(index)).collect();
    let from_source = match crate::Bvh::build(&source) {
        Ok(hierarchy) => hierarchy,
        Err(error) => panic!("sphere source must build: {error}"),
    };
    let from_boxes = match crate::Bvh::build(&materialised[..]) {
        Ok(hierarchy) => hierarchy,
        Err(error) => panic!("box slice must build: {error}"),
    };
    assert_eq!(from_source, from_boxes);
}
