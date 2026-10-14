use super::*;

#[test]
fn units_order_from_the_shortest_to_the_longest() {
    let mut sorted = TimeUnit::ALL;
    sorted.sort();
    assert_eq!(sorted, TimeUnit::ALL);
}

#[test]
fn each_unit_is_exactly_one_thousand_of_the_next_longer_unit() {
    for pair in TimeUnit::ALL.windows(2) {
        let [shorter, longer] = pair else {
            panic!("windows(2) yields pairs")
        };
        let ratio = longer.seconds() / shorter.seconds();
        assert!((ratio - 1000.0).abs() <= 1000.0 * 1.0e-12, "{ratio}");
    }
}

#[test]
fn every_symbol_parses_back_to_its_own_unit() {
    for unit in TimeUnit::ALL {
        assert_eq!(TimeUnit::from_symbol(unit.symbol()), Some(unit));
    }
}

#[test]
fn the_ascii_microsecond_alias_parses_to_microseconds() {
    assert_eq!(TimeUnit::from_symbol("us"), Some(TimeUnit::Microsecond));
}

#[test]
fn an_unrecognised_symbol_is_rejected_rather_than_read_as_seconds() {
    assert_eq!(TimeUnit::from_symbol("ps "), None);
    assert_eq!(TimeUnit::from_symbol("min"), None);
    assert_eq!(TimeUnit::from_symbol(""), None);
}
