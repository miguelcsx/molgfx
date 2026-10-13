use super::{lookup, names};

#[test]
fn every_catalogued_ramp_fits_a_ramp_and_has_a_unique_name() {
    let all = names();
    for (index, name) in all.iter().enumerate() {
        assert!(!all[..index].contains(name), "{name} appears twice");
        let Some((ramp, _)) = lookup(name) else {
            panic!("{name} does not resolve")
        };
        assert!(
            (2..=molgfx_core::MAX_RAMP_STOPS).contains(&ramp.anchors.len()),
            "{name} has {} anchors",
            ramp.anchors.len()
        );
    }
}

#[test]
fn the_reversing_suffix_reads_the_same_ramp_backwards() {
    let (forward, flipped) = lookup("viridis").unwrap_or_else(|| panic!("viridis exists"));
    assert!(!flipped);
    let (reverse, flipped) = lookup("viridis_r").unwrap_or_else(|| panic!("viridis_r exists"));
    assert!(flipped);
    assert!(std::ptr::eq(forward, reverse));
    let mut colors = forward.colors(false);
    colors.reverse();
    assert_eq!(colors, reverse.colors(true));
}

#[test]
fn a_name_that_is_not_catalogued_does_not_resolve() {
    assert!(lookup("nope").is_none());
    assert!(lookup("nope_r").is_none());
}

/// The anchors the `plddt` ramp shipped with before it shared constants with
/// `PLDDT_BANDS`; the colouring of every published model depends on them.
const SHIPPED_PLDDT: [u32; 11] = [
    0xff_7d_45, 0xff_7d_45, 0xff_7d_45, 0xff_7d_45, 0xff_7d_45, 0xff_db_13, 0xff_db_13, 0x65_cb_f3,
    0x65_cb_f3, 0x00_53_d6, 0x00_53_d6,
];

#[test]
fn the_plddt_ramp_keeps_its_shipped_anchors() {
    let Some((ramp, flipped)) = lookup("plddt") else {
        panic!("plddt exists")
    };
    assert!(!flipped);
    assert_eq!(ramp.anchors, SHIPPED_PLDDT);
}

#[test]
fn the_plddt_ramp_is_flat_inside_a_band_and_blends_at_each_band_edge() {
    let Some((ramp, _)) = lookup("plddt") else {
        panic!("plddt exists")
    };
    let colors: Vec<_> = ramp.colors(false).iter().map(|c| c.native()).collect();
    let Ok(scalar) = molgfx_core::ScalarRamp::evenly([0.0, 100.0], &colors) else {
        panic!("eleven stops fit a ramp")
    };
    let at = |score: f32| {
        let got = scalar.sample(score, colors[0]);
        [got.r, got.g, got.b]
    };
    let channels = |hex: u32| {
        let [_, r, g, b] = hex.to_be_bytes();
        [r, g, b]
    };
    // Band boundaries sit on anchors, so they are exact.
    for (score, index) in [
        (0.0, 0),
        (40.0, 4),
        (50.0, 5),
        (60.0, 6),
        (70.0, 7),
        (80.0, 8),
        (90.0, 9),
        (100.0, 10),
    ] {
        assert_eq!(at(score), channels(SHIPPED_PLDDT[index]), "score {score}");
    }
    // Mid-points of the three blended edges are the mean of the neighbours,
    // to within one eight-bit step.
    for (score, low, high) in [(45.0, 4, 5), (65.0, 6, 7), (85.0, 8, 9)] {
        let (a, b) = (channels(SHIPPED_PLDDT[low]), channels(SHIPPED_PLDDT[high]));
        let got = at(score);
        for channel in 0..3 {
            let mean = i16::midpoint(i16::from(a[channel]), i16::from(b[channel]));
            assert!(
                (i16::from(got[channel]) - mean).abs() <= 1,
                "score {score} channel {channel}: {} vs {mean}",
                got[channel]
            );
        }
    }
}
