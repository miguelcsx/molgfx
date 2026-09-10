use super::*;

fn group(family: u32, shape: u32, translucent: bool) -> PrimitiveDrawGroup {
    PrimitiveDrawGroup {
        family,
        shape,
        translucent,
        first: 11,
        len: 7,
    }
}

#[test]
fn opaque_analytic_classes_route_to_their_exact_shadow_kinds() {
    let cases = [
        (group(FAMILY_ELLIPSOID, 0, false), KIND_ELLIPSOID),
        (group(FAMILY_BOX, 0, false), KIND_BOX),
        (
            group(FAMILY_POLYGON, POLYGON_PENTAGON, false),
            KIND_POLYGON_PENTAGON,
        ),
        (
            group(FAMILY_POLYGON, POLYGON_HEXAGON, false),
            KIND_POLYGON_HEXAGON,
        ),
        (group(FAMILY_PARTICLE, PARTICLE_SPHERE, false), KIND_SPHERE),
        (
            group(FAMILY_PARTICLE, PARTICLE_CYLINDER, false),
            KIND_CYLINDER,
        ),
        (
            group(FAMILY_PARTICLE, PARTICLE_SPHEROCYLINDER, false),
            KIND_SPHEROCYLINDER,
        ),
        (group(FAMILY_PARTICLE, PARTICLE_CIRCLE, false), KIND_CIRCLE),
        (group(FAMILY_PARTICLE, PARTICLE_SQUARE, false), KIND_SQUARE),
        (
            group(FAMILY_PARTICLE, PARTICLE_SUPERQUADRIC, false),
            KIND_SUPERQUADRIC,
        ),
    ];

    for (group, expected) in cases {
        assert_eq!(shadow_kind(&group), Some(expected));
    }
}

#[test]
fn gaussian_and_every_translucent_group_are_excluded_from_shadows() {
    assert_eq!(
        shadow_kind(&group(FAMILY_PARTICLE, PARTICLE_GAUSSIAN, false)),
        None,
    );
    assert_eq!(shadow_kind(&group(FAMILY_ELLIPSOID, 0, true)), None);
    assert_eq!(
        shadow_kind(&group(FAMILY_PARTICLE, PARTICLE_SUPERQUADRIC, true)),
        None,
    );
}

#[test]
fn unmapped_shapes_never_fall_back_to_an_incorrect_intersection() {
    assert_eq!(shadow_kind(&group(FAMILY_POLYGON, 4, false)), None);
    assert_eq!(shadow_kind(&group(FAMILY_PARTICLE, 99, false)), None);
    assert_eq!(shadow_kind(&group(99, 0, false)), None);
}
