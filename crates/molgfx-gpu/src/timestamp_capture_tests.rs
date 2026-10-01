use super::*;

fn capture(max_passes: u32) -> PassTimestampCapture<()> {
    PassTimestampCapture {
        queries: Some(()),
        records: Vec::with_capacity(max_passes as usize),
        max_passes,
        next_query: 0,
        passes_seen: 0,
        sample: None,
    }
}

#[test]
fn repeated_pass_labels_and_samples_receive_distinct_query_pairs() {
    let mut capture = capture(3);
    capture.set_sample(Some(4));
    assert_eq!(
        capture.record_pass("repeat", TimestampPassKind::Render, false),
        Some(TimestampQueryPair {
            beginning: 0,
            end: 1
        })
    );
    assert_eq!(
        capture.record_pass("repeat", TimestampPassKind::Compute, false),
        Some(TimestampQueryPair {
            beginning: 2,
            end: 3
        })
    );
    capture.set_sample(Some(5));
    assert_eq!(
        capture.record_pass("repeat", TimestampPassKind::Render, false),
        Some(TimestampQueryPair {
            beginning: 4,
            end: 5
        })
    );
    assert_eq!(capture.query_range(), 0..6);
    assert_eq!(
        capture
            .records()
            .iter()
            .map(|r| (r.occurrence, r.sample, r.kind))
            .collect::<Vec<_>>(),
        vec![
            (0, Some(4), TimestampPassKind::Render),
            (1, Some(4), TimestampPassKind::Compute),
            (2, Some(5), TimestampPassKind::Render),
        ]
    );
    assert_eq!(capture.incomplete(), None);
}

#[test]
fn overflow_never_reuses_queries_and_reports_every_omitted_occurrence() {
    let mut capture = capture(1);
    capture.record_pass("first", TimestampPassKind::Compute, false);
    for _ in 0..3 {
        assert_eq!(
            capture.record_pass("overflow", TimestampPassKind::Render, false),
            None
        );
    }
    assert_eq!(capture.query_range(), 0..2);
    assert_eq!(capture.records()[0].label, "first");
    assert_eq!(capture.passes_seen(), 4);
    assert_eq!(
        capture.incomplete(),
        Some(TimestampCaptureIncomplete::CapacityExceeded {
            capacity: 1,
            omitted_passes: 3
        })
    );
    capture.reset();
    assert_eq!(
        capture.record_pass("next submission", TimestampPassKind::Compute, false),
        Some(TimestampQueryPair {
            beginning: 0,
            end: 1
        })
    );
    assert_eq!(capture.incomplete(), None);
    assert_eq!(capture.records()[0].occurrence, 0);
    assert_eq!(capture.records()[0].sample, None);
}

#[test]
fn descriptor_owned_writes_are_absent_from_the_capture_resolve_range() {
    let mut capture = capture(3);
    assert_eq!(
        capture.record_pass("caller timed", TimestampPassKind::Render, true),
        None
    );
    assert_eq!(
        capture.records()[0].queries,
        Err(PassTimestampAbsence::DescriptorTimestampWrites)
    );
    assert_eq!(capture.query_range(), 0..0);
    assert_eq!(
        capture.record_pass("captured", TimestampPassKind::Compute, false),
        Some(TimestampQueryPair {
            beginning: 0,
            end: 1
        })
    );
    assert_eq!(capture.query_range(), 0..2);
    assert_eq!(capture.records()[1].occurrence, 1);
}

#[test]
fn metadata_only_capture_counts_passes_without_query_slots_or_fake_timings() {
    let mut capture = PassTimestampCapture::<()>::metadata_only(2).expect("metadata allocates");
    capture.set_sample(Some(3));
    assert_eq!(
        capture.record_pass("render", TimestampPassKind::Render, false),
        None
    );
    assert_eq!(
        capture.record_pass("compute", TimestampPassKind::Compute, false),
        None
    );
    assert_eq!(
        capture.records()[0].queries,
        Err(PassTimestampAbsence::Unsupported)
    );
    assert_eq!(capture.records()[1].sample, Some(3));
    assert_eq!(capture.records()[1].kind, TimestampPassKind::Compute);
    assert_eq!(capture.query_range(), 0..0);
    assert_eq!(capture.query_capacity(), 0);
    assert!(capture.queries().is_none());
    capture.record_pass("overflow", TimestampPassKind::Render, false);
    assert_eq!(
        capture.incomplete(),
        Some(TimestampCaptureIncomplete::CapacityExceeded {
            capacity: 2,
            omitted_passes: 1
        })
    );
}
