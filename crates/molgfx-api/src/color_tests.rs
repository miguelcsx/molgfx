use crate::color::{Color, property};

#[test]
fn property_legends_keep_domain_units_and_missing_color() {
    let missing = Color::rgb(120, 120, 120);
    let spec = property(
        "electrostatic potential",
        "coolwarm",
        [-2.0, 2.0],
        Some("kT/e".into()),
        missing,
    );
    let Some(legend) = spec.legend() else {
        panic!("property color must generate a legend")
    };
    assert_eq!(legend.title.as_ref(), "electrostatic potential");
    assert_eq!(legend.units.as_deref(), Some("kT/e"));
    assert_eq!(legend.missing, missing);
    assert_eq!(legend.stops[0].value.to_bits(), (-2.0_f32).to_bits());
    assert_eq!(legend.stops[1].value.to_bits(), 0.0_f32.to_bits());
    assert_eq!(legend.stops[2].value.to_bits(), 2.0_f32.to_bits());
}

#[test]
fn property_domains_must_be_finite_and_increasing() {
    let spec = property(
        "confidence",
        "viridis",
        [1.0, 1.0],
        None,
        Color::rgb(0, 0, 0),
    );
    assert!(spec.validate().is_err());
}
