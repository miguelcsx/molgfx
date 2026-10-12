use crate::color::{AtomMetric, Color, PLDDT_BANDS, metric, property};
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
    // One legend stop per anchor of the ramp, spanning the declared domain.
    assert_eq!(legend.stops.len(), 8);
    assert_eq!(legend.stops[0].value.to_bits(), (-2.0_f32).to_bits());
    assert_eq!(legend.stops[7].value.to_bits(), 2.0_f32.to_bits());
    assert!(
        legend
            .stops
            .windows(2)
            .all(|pair| pair[0].value < pair[1].value)
    );
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

#[test]
fn a_legend_colors_a_value_the_way_its_ramp_does_and_clamps_beyond_the_ends() {
    let spec = metric(AtomMetric::BFactor).with_domain([10.0, 30.0]);
    let Some(legend) = spec.legend() else {
        panic!("a metric color must generate a legend")
    };
    let (first, last) = (
        legend.stops[0].color,
        legend.stops[legend.stops.len() - 1].color,
    );
    assert_eq!(legend.color_at(10.0), first);
    assert_eq!(legend.color_at(-100.0), first);
    assert_eq!(legend.color_at(30.0), last);
    assert_eq!(legend.color_at(1.0e6), last);
    assert_eq!(legend.color_at(f32::NAN), legend.missing);
    let middle = &legend.stops[legend.stops.len() / 2];
    assert_eq!(legend.color_at(middle.value), middle.color);
}

#[test]
fn the_plddt_bands_are_the_anchors_of_the_plddt_ramp() {
    let Some(legend) = metric(AtomMetric::Plddt).legend() else {
        panic!("a metric color must generate a legend")
    };
    for band in PLDDT_BANDS {
        // Score ten points inside a band is the band's own colour.
        let inside = if band.lower == 0 {
            0.0
        } else {
            f32::from(band.lower) + 10.0
        };
        let [r, g, b] = band.rgb;
        assert_eq!(
            legend.color_at(inside.min(100.0)),
            Color::rgb(r, g, b),
            "{}",
            band.label
        );
    }
}
