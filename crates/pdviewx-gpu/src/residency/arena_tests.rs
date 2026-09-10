use super::{ArenaError, PagedArena};

#[test]
fn first_fit_reuses_the_lowest_suitable_address() {
    let Ok(mut arena) = PagedArena::new(256, 8) else {
        panic!("arena configuration is valid")
    };
    let Ok(first) = arena.allocate(300) else {
        panic!("first allocation fits")
    };
    let Ok(second) = arena.allocate(256) else {
        panic!("second allocation fits")
    };
    assert!(arena.release(first).is_ok());
    let Ok(reused) = arena.allocate(128) else {
        panic!("released run is reusable")
    };
    assert_eq!(reused.start_page(), 0);
    assert_eq!(second.start_page(), 2);
}

#[test]
fn adjacent_runs_coalesce_into_a_larger_allocation() {
    let Ok(mut arena) = PagedArena::new(64, 4) else {
        panic!("arena configuration is valid")
    };
    let Ok(left) = arena.allocate(64) else {
        panic!("left allocation fits")
    };
    let Ok(right) = arena.allocate(64) else {
        panic!("right allocation fits")
    };
    assert!(arena.release(left).is_ok());
    assert!(arena.release(right).is_ok());
    let Ok(joined) = arena.allocate(256) else {
        panic!("coalesced run fits")
    };
    assert_eq!(joined.page_count(), 4);
}

#[test]
fn stale_handles_cannot_release_reused_pages() {
    let Ok(mut arena) = PagedArena::new(64, 1) else {
        panic!("arena configuration is valid")
    };
    let Ok(old) = arena.allocate(1) else {
        panic!("allocation fits")
    };
    assert!(arena.release(old).is_ok());
    let Ok(current) = arena.allocate(1) else {
        panic!("replacement fits")
    };
    assert_eq!(arena.release(old), Err(ArenaError::StaleAllocation));
    assert!(arena.release(current).is_ok());
}

#[test]
fn arena_metadata_does_not_allocate_after_construction() {
    let Ok(mut arena) = PagedArena::new(64, 32) else {
        panic!("arena configuration is valid")
    };
    let warm_metrics = arena.metrics();
    for _ in 0..128 {
        let Ok(allocation) = arena.allocate(65) else {
            panic!("allocation fits")
        };
        assert!(arena.release(allocation).is_ok());
    }
    assert_eq!(
        arena.metrics().host_allocation_events,
        warm_metrics.host_allocation_events
    );
    assert_eq!(arena.metrics().resident_bytes, 0);
}

#[test]
fn capacity_failures_report_real_stall_metrics() {
    let Ok(mut arena) = PagedArena::new(64, 1) else {
        panic!("arena configuration is valid")
    };
    assert!(arena.allocate(65).is_err());
    assert_eq!(arena.metrics().allocation_stalls, 1);
    assert_eq!(arena.metrics().stalled_bytes, 65);
}

#[test]
fn growth_preserves_offsets_and_appends_free_capacity() {
    let Ok(mut arena) = PagedArena::new(64, 2) else {
        panic!("arena configuration is valid")
    };
    let Ok(first) = arena.allocate(65) else {
        panic!("initial allocation fits")
    };
    assert!(arena.grow(3).is_ok());
    let Ok(second) = arena.allocate(128) else {
        panic!("appended allocation fits")
    };
    assert_eq!(first.byte_offset(), 0);
    assert_eq!(second.byte_offset(), 128);
    assert_eq!(arena.page_count(), 5);
    assert!(arena.release(first).is_ok());
    assert!(arena.release(second).is_ok());
}

#[test]
fn zero_page_growth_is_a_typed_error() {
    let Ok(mut arena) = PagedArena::new(64, 1) else {
        panic!("arena configuration is valid")
    };
    assert_eq!(arena.grow(0), Err(ArenaError::EmptyGrowth));
}
