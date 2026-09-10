use super::*;

#[test]
fn ledger_rejects_an_allocation_before_crossing_the_total_limit() {
    let ledger = ResourceLedger::new(Some(12));
    ledger
        .reserve_buffer(8, "buffer")
        .unwrap_or_else(|error| panic!("first allocation fits: {error}"));

    assert!(matches!(
        ledger.reserve_texture(5, "texture"),
        Err(GpuError::LimitExceeded {
            resource: "texture",
            limit: 12
        })
    ));
    assert_eq!(ledger.usage().buffer_bytes, 8);
    assert_eq!(ledger.usage().texture_bytes, 0);
    assert_eq!(ledger.usage().peak_bytes, 8);
}

#[test]
fn ledger_reports_stable_buffer_and_texture_categories() {
    let ledger = ResourceLedger::new(None);
    ledger
        .reserve_buffer(7, "buffer")
        .unwrap_or_else(|error| panic!("buffer allocation fits: {error}"));
    ledger
        .reserve_texture(11, "texture")
        .unwrap_or_else(|error| panic!("texture allocation fits: {error}"));

    assert_eq!(
        ledger.usage(),
        ResourceMemory {
            buffer_bytes: 7,
            texture_bytes: 11,
            peak_bytes: 18,
        }
    );
}
