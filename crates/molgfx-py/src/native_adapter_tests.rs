use super::{decode_secondary_structure, payload_structure};

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

#[test]
fn an_unreadable_payload_is_a_value_error_rather_than_a_missing_structure() {
    pyo3::Python::initialize();
    let error = payload_structure(b"not a bcif document").unwrap_err();
    assert!(error.to_string().contains("not a readable structure"));
}
