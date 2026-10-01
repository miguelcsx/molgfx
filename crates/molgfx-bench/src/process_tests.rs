use super::*;

#[test]
fn darwin_rss_preserves_bytes_and_ignores_different_footprint_metric() {
    let raw = "  0.10 real 0.02 user 0.01 sys\n  1048576 maximum resident set size\n  524288 peak memory footprint\n";
    assert_eq!(darwin_peak_rss(raw).expect("RSS"), 1_048_576);
}

#[test]
fn darwin_rss_rejects_missing_malformed_and_duplicate_measurements() {
    for raw in [
        "",
        "  512 peak memory footprint\n",
        "  unknown maximum resident set size\n",
        "  -1 maximum resident set size\n",
        "  20 maximum resident set size\n  30 maximum resident set size\n",
    ] {
        assert!(darwin_peak_rss(raw).is_err(), "accepted {raw:?}");
    }
}
