use super::*;

#[test]
fn every_snfg_shape_lowers_to_an_analytic_family() {
    let cases = [
        (CarbohydrateShape::Sphere, FAMILY_PARTICLE, 0),
        (CarbohydrateShape::Cube, FAMILY_BOX, 0),
        (CarbohydrateShape::Bar, FAMILY_BOX, 0),
        (CarbohydrateShape::Diamond, FAMILY_POLYGON, POLYGON_DIAMOND),
        (
            CarbohydrateShape::Triangle,
            FAMILY_POLYGON,
            POLYGON_TRIANGLE,
        ),
        (CarbohydrateShape::Star, FAMILY_POLYGON, POLYGON_STAR),
        (CarbohydrateShape::Hexagon, FAMILY_POLYGON, 1),
        (CarbohydrateShape::Pentagon, FAMILY_POLYGON, 5),
    ];
    for (shape, family, code) in cases {
        assert_eq!(carbohydrate_class(shape), (family, code), "{shape:?}");
    }
}

#[test]
fn legacy_symbols_keep_their_polygon_codes() {
    assert_eq!(
        carbohydrate_class(CarbohydrateShape::Neu5Ac),
        (FAMILY_POLYGON, 6)
    );
    assert_eq!(
        carbohydrate_class(CarbohydrateShape::Fuc),
        (FAMILY_POLYGON, 4)
    );
}
