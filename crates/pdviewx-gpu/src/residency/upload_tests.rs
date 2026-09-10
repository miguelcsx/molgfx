use super::{FenceValue, UploadBackpressure, UploadRing, UploadRingConfig, UploadState};

#[test]
fn configuration_validation_matches_allocation_without_committing_storage() {
    let config = UploadRingConfig {
        capacity_bytes: 256,
        ticket_capacity: 8,
        epoch_budget_bytes: 512,
        in_flight_budget_bytes: 256,
        alignment: 16,
    };
    assert!(config.validate().is_ok());
    assert!(UploadRing::new(config).is_ok());
    for invalid in [
        UploadRingConfig {
            capacity_bytes: 0,
            ..config
        },
        UploadRingConfig {
            ticket_capacity: 0,
            ..config
        },
        UploadRingConfig {
            epoch_budget_bytes: 0,
            ..config
        },
        UploadRingConfig {
            in_flight_budget_bytes: 0,
            ..config
        },
        UploadRingConfig {
            in_flight_budget_bytes: 257,
            ..config
        },
        UploadRingConfig {
            alignment: 0,
            ..config
        },
        UploadRingConfig {
            alignment: 3,
            ..config
        },
    ] {
        assert_eq!(
            invalid.validate(),
            Err(UploadBackpressure::InvalidConfiguration)
        );
        assert!(matches!(
            UploadRing::new(invalid),
            Err(UploadBackpressure::InvalidConfiguration)
        ));
    }
}

fn ring(capacity_bytes: usize, epoch_budget_bytes: usize) -> UploadRing {
    let result = UploadRing::new(UploadRingConfig {
        capacity_bytes,
        ticket_capacity: 8,
        epoch_budget_bytes,
        in_flight_budget_bytes: capacity_bytes,
        alignment: 16,
    });
    let Ok(ring) = result else {
        panic!("ring configuration is valid")
    };
    ring
}

#[test]
fn committed_bytes_retire_only_after_their_fence() {
    let mut ring = ring(128, 128);
    let Ok(reservation) = ring.reserve(24) else {
        panic!("reservation fits")
    };
    let Ok(bytes) = ring.bytes_mut(reservation) else {
        panic!("reserved bytes are writable")
    };
    bytes.fill(7);
    assert!(ring.commit(reservation.ticket()).is_ok());
    assert!(ring.submit(reservation.ticket(), FenceValue(4)).is_ok());
    assert_eq!(ring.retire(FenceValue(3)).tickets, 0);
    assert_eq!(
        ring.state(reservation.ticket()),
        Some(UploadState::InFlight(FenceValue(4)))
    );
    let retired = ring.retire(FenceValue(4));
    assert_eq!(retired.tickets, 1);
    assert_eq!(retired.bytes, 24);
    assert_eq!(ring.metrics().occupied_bytes, 0);
}

#[test]
fn ring_wrap_never_splits_a_payload() {
    let mut ring = ring(64, 128);
    let Ok(first) = ring.reserve(48) else {
        panic!("first reservation fits")
    };
    assert!(ring.commit(first.ticket()).is_ok());
    assert!(ring.submit(first.ticket(), FenceValue(1)).is_ok());
    assert_eq!(ring.retire(FenceValue(1)).tickets, 1);
    let Ok(wrapped) = ring.reserve(32) else {
        panic!("wrapped reservation fits")
    };
    assert_eq!(wrapped.offset(), 0);
    assert_eq!(wrapped.len(), 32);
}

#[test]
fn epoch_budget_reports_backpressure_and_stalled_bytes() {
    let mut ring = ring(128, 32);
    assert!(ring.reserve(24).is_ok());
    assert_eq!(ring.reserve(16), Err(UploadBackpressure::EpochBudget));
    assert_eq!(ring.metrics().stall_events, 1);
    assert_eq!(ring.metrics().stalled_bytes, 16);
    ring.begin_epoch();
    assert!(ring.reserve(16).is_ok());
}

#[test]
fn in_flight_budget_reopens_only_after_fence_retirement() {
    let result = UploadRing::new(UploadRingConfig {
        capacity_bytes: 128,
        ticket_capacity: 4,
        epoch_budget_bytes: 128,
        in_flight_budget_bytes: 32,
        alignment: 16,
    });
    let Ok(mut ring) = result else {
        panic!("ring configuration is valid")
    };
    let Ok(first) = ring.reserve(24) else {
        panic!("first upload fits")
    };
    let Ok(second) = ring.reserve(24) else {
        panic!("second upload fits staging")
    };
    assert!(ring.commit(first.ticket()).is_ok());
    assert!(ring.commit(second.ticket()).is_ok());
    assert!(ring.submit(first.ticket(), FenceValue(1)).is_ok());
    assert_eq!(
        ring.submit(second.ticket(), FenceValue(2)),
        Err(UploadBackpressure::InFlightBudget)
    );
    assert_eq!(ring.retire(FenceValue(1)).tickets, 1);
    assert!(ring.submit(second.ticket(), FenceValue(2)).is_ok());
    assert_eq!(ring.metrics().peak_in_flight_bytes, 24);
}

#[test]
fn an_older_fence_blocks_reuse_even_when_newer_work_completed() {
    let mut ring = ring(64, 128);
    let Ok(oldest) = ring.reserve(32) else {
        panic!("oldest reservation fits")
    };
    let Ok(newest) = ring.reserve(32) else {
        panic!("newest reservation fits")
    };
    assert!(ring.commit(oldest.ticket()).is_ok());
    assert!(ring.commit(newest.ticket()).is_ok());
    assert!(ring.submit(oldest.ticket(), FenceValue(9)).is_ok());
    assert!(ring.submit(newest.ticket(), FenceValue(2)).is_ok());
    assert_eq!(ring.retire(FenceValue(2)).tickets, 0);
    assert_eq!(ring.reserve(1), Err(UploadBackpressure::RingFull));
}

#[test]
fn ring_storage_and_allocation_metrics_stay_stable_after_warm_up() {
    let mut ring = ring(64, 64);
    let staging_pointer = ring.staging_bytes().as_ptr();
    let allocation_events = ring.metrics().host_allocation_events;
    for fence in 1..65 {
        ring.begin_epoch();
        let Ok(reservation) = ring.reserve(32) else {
            panic!("reservation fits")
        };
        assert!(ring.commit(reservation.ticket()).is_ok());
        assert!(ring.submit(reservation.ticket(), FenceValue(fence)).is_ok());
        assert_eq!(ring.retire(FenceValue(fence)).tickets, 1);
        assert_eq!(ring.staging_bytes().as_ptr(), staging_pointer);
    }
    assert_eq!(ring.metrics().host_allocation_events, allocation_events);
    assert_eq!(ring.metrics().bytes_retired, 64 * 32);
}

#[test]
fn cancellation_reclaims_only_the_fifo_prefix() {
    let mut ring = ring(64, 64);
    let Ok(first) = ring.reserve(16) else {
        panic!("first reservation fits")
    };
    let Ok(second) = ring.reserve(16) else {
        panic!("second reservation fits")
    };
    assert!(ring.cancel(second.ticket()).is_ok());
    assert_eq!(ring.metrics().active_tickets, 2);
    assert!(ring.cancel(first.ticket()).is_ok());
    assert_eq!(ring.metrics().active_tickets, 0);
    assert_eq!(ring.metrics().bytes_cancelled, 32);
}
