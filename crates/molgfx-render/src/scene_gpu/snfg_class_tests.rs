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

#[test]
fn split_modes_follow_the_snfg_division_of_each_shape() {
    let white = Rgba8::opaque(255, 255, 255);
    let cases = [
        (CarbohydrateShape::Cube, 1.0),
        (CarbohydrateShape::Diamond, 2.0),
        (CarbohydrateShape::Triangle, 3.0),
    ];
    for (shape, mode) in cases {
        assert_eq!(
            bits(secondary_split(shape, Some(white))),
            bits([1.0, 1.0, 1.0, mode])
        );
        assert_eq!(bits(secondary_split(shape, None)), bits([0.0; 4]));
    }
    assert_eq!(
        bits(secondary_split(CarbohydrateShape::Sphere, Some(white))),
        bits([0.0; 4])
    );
}

fn bits(value: [f32; 4]) -> [u32; 4] {
    value.map(f32::to_bits)
}
