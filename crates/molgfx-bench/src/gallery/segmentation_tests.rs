use super::label;

#[test]
fn threshold_boundaries_produce_exact_integer_categories() {
    let thresholds = [0.05, 0.2, 0.5];
    assert_eq!(label(0.0, thresholds), 0);
    assert_eq!(label(0.05, thresholds), 3);
    assert_eq!(label(0.1, thresholds), 3);
    assert_eq!(label(0.2, thresholds), 2);
    assert_eq!(label(0.4, thresholds), 2);
    assert_eq!(label(0.5, thresholds), 1);
    assert_eq!(label(1.0, thresholds), 1);
}
