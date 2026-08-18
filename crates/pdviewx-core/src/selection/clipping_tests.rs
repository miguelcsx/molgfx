use super::*;

#[test]
fn a_plane_and_slab_retain_the_declared_half_spaces() {
    let plane = match ClipPlane::from_point_normal(Vec3::ZERO, Vec3::X) {
        Ok(plane) => plane,
        Err(error) => panic!("plane builds: {error}"),
    };
    assert!(plane.signed_distance(Vec3::X) > 0.0);
    assert!(plane.reversed().signed_distance(Vec3::X) < 0.0);
    let slab = match ClipSet::slab(Vec3::ZERO, Vec3::Z, 2.0) {
        Ok(slab) => slab,
        Err(error) => panic!("slab builds: {error}"),
    };
    assert!(slab.contains(Vec3::new(0.0, 0.0, 0.9)));
    assert!(!slab.contains(Vec3::new(0.0, 0.0, 1.1)));
    assert_eq!(slab.cap(), ClipCap::Open);
    assert_eq!(slab.with_cap(ClipCap::Solid).cap(), ClipCap::Solid);
}

#[test]
fn malformed_clipping_is_a_typed_error() {
    let Err(normal) = ClipPlane::from_point_normal(Vec3::ZERO, Vec3::ZERO) else {
        panic!("zero normal is invalid")
    };
    assert_eq!(normal.code(), "PDVIEWX-E0033");
    let Err(thickness) = ClipSet::slab(Vec3::ZERO, Vec3::Z, 0.0) else {
        panic!("zero slab thickness is invalid")
    };
    assert_eq!(thickness.code(), "PDVIEWX-E0033");
}
