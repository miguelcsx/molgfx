use super::*;
use crate::fixture;

#[test]
fn the_coordinate_slice_is_the_parsers_own_buffer_not_a_copy() {
    let structure = fixture::structure();
    let Some(coords) = CoordRef::new(&structure, pdbiox::ModelIndex::new(0)) else {
        panic!("fixture has a dense model 0")
    };
    // Pointer identity: the seam hands out the same memory the parser owns.
    assert_eq!(coords.slice().as_ptr(), structure.positions().as_ptr());
    assert_eq!(coords.len(), structure.positions().len());
}

#[test]
fn coordinate_bytes_are_twelve_per_atom_over_the_same_memory() {
    let structure = fixture::structure();
    let Some(coords) = CoordRef::new(&structure, pdbiox::ModelIndex::new(0)) else {
        panic!("fixture has a dense model 0")
    };
    let bytes = coords.as_bytes();
    assert_eq!(bytes.len(), coords.len() * 12);
    assert_eq!(bytes.as_ptr(), structure.positions().as_ptr().cast());
}

#[test]
fn a_reference_to_a_missing_model_is_refused_up_front() {
    let structure = fixture::structure();
    assert!(CoordRef::new(&structure, pdbiox::ModelIndex::new(42)).is_none());
}

#[test]
fn the_coordinate_bound_covers_every_fixture_atom() {
    let structure = fixture::structure();
    let Some(coords) = CoordRef::new(&structure, pdbiox::ModelIndex::new(0)) else {
        panic!("fixture has a dense model 0")
    };
    let aabb = coords.aabb();
    for p in coords.slice() {
        assert!(aabb.min.x <= p[0] && p[0] <= aabb.max.x);
        assert!(aabb.min.y <= p[1] && p[1] <= aabb.max.y);
        assert!(aabb.min.z <= p[2] && p[2] <= aabb.max.z);
    }
}

#[test]
fn the_generation_counter_is_stable_while_nothing_changes() {
    let structure = fixture::structure();
    let Some(coords) = CoordRef::new(&structure, pdbiox::ModelIndex::new(0)) else {
        panic!("fixture has a dense model 0")
    };
    assert_eq!(coords.generation(), coords.generation());
}
