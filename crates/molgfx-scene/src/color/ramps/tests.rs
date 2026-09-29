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
