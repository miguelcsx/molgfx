use crate::color::{Color, property};
use crate::{DataSource, ScalarPropertyBinding, StructureId};
use std::sync::Arc;

fn scalar_property(name: &str) -> crate::ScalarProperty {
    ScalarPropertyBinding::new(
        StructureId::new(1),
        name,
        DataSource::new("test-property"),
        Arc::from([0.0_f32, 1.0]),
    )
    .into_parts()
    .0
}

#[test]
fn property_legends_keep_domain_units_and_missing_color() {
    let missing = Color::rgb(120, 120, 120);
    let spec = property(
        scalar_property("electrostatic potential"),
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
        scalar_property("confidence"),
        "viridis",
        [1.0, 1.0],
        None,
        Color::rgb(0, 0, 0),
    );
    assert!(spec.validate().is_err());
}

#[test]
fn an_unknown_ramp_is_rejected_instead_of_silently_substituted() {
    let spec = property(
        scalar_property("charge"),
        "not-a-ramp",
        [-1.0, 1.0],
        None,
        Color::rgb(0, 0, 0),
    );
    let Err(error) = spec.validate() else {
        panic!("an unknown ramp must not validate")
    };
    let message = error.to_string();
    assert!(message.contains("not-a-ramp"), "{message}");
    assert!(message.contains("viridis"), "{message}");
}

#[test]
fn every_named_ramp_resolves() {
    for name in ["viridis", "plasma", "coolwarm"] {
        let spec = property(
            scalar_property("charge"),
            name,
            [-1.0, 1.0],
            None,
            Color::rgb(0, 0, 0),
        );
        if let Err(error) = spec.validate() {
            panic!("{name} must resolve: {error}")
        }
    }
}
