use super::fallback;

#[test]
fn an_absent_value_yields_the_stated_default() {
    assert_eq!(fallback(None, 7), 7);
    assert_eq!(fallback(Vec::<u8>::new(), 3), 3);
}

#[test]
fn a_present_value_wins_over_the_default() {
    assert_eq!(fallback(Some(2), 7), 2);
    assert_eq!(fallback([1, 2, 3], 7), 3);
}
