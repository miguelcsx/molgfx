use super::decode_secondary_structure;

#[test]
fn native_secondary_codes_retain_all_exact_states_and_reject_invalid_codes() {
    for code in 0..=10 {
        let state = decode_secondary_structure(code).unwrap();
        assert_eq!(state.code(), code);
    }
    for code in 11..=u8::MAX {
        assert!(decode_secondary_structure(code).is_err());
    }
}
