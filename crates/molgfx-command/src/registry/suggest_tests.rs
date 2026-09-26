use super::suggest;

#[test]
fn a_one_letter_slip_suggests_the_intended_word() {
    assert_eq!(
        suggest("cartoom", ["cartoon", "spacefill"]),
        Some("cartoon".to_owned())
    );
}

#[test]
fn a_transposition_counts_as_one_edit() {
    assert_eq!(
        suggest("slect", ["select", "show"]),
        Some("select".to_owned())
    );
    assert_eq!(suggest("shwo", ["select", "show"]), Some("show".to_owned()));
}

#[test]
fn an_unrelated_word_suggests_nothing() {
    assert_eq!(suggest("banana", ["cartoon", "spacefill"]), None);
}

#[test]
fn an_exact_match_is_not_a_suggestion() {
    assert_eq!(suggest("show", ["show"]), None);
}

#[test]
fn an_overlong_word_is_never_compared() {
    let word = "a".repeat(65);
    assert_eq!(suggest(&word, [word.as_str()]), None);
}
