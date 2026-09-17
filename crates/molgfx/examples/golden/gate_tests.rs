use super::verify_comparison;

#[test]
fn unverified_reference_identity_never_passes_the_integration_gate() {
    for failures in [0, 12] {
        let error = verify_comparison(false, failures, true)
            .expect_err("uncertified comparisons cannot close the gate");
        assert!(error.to_string().contains("UNCERTIFIED"));
    }
}

#[test]
fn matching_reference_fingerprint_still_rejects_image_drift() {
    assert!(verify_comparison(true, 0, true).is_ok());
    assert!(verify_comparison(true, 1, true).is_err());
    assert!(verify_comparison(true, 1, false).is_err());
}

#[test]
fn diagnostic_comparisons_can_report_drift_without_certifying_a_release() {
    assert!(verify_comparison(false, 12, false).is_ok());
}
