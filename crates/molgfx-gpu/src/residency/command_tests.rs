use super::CommandScratch;

#[test]
fn clear_reuses_the_same_command_storage() {
    let mut scratch = CommandScratch::new(4);
    assert!(scratch.push(1_u32).is_ok());
    let pointer = scratch.as_slice().as_ptr();
    let allocation_events = scratch.metrics().host_allocation_events;
    for value in 0..64 {
        scratch.clear();
        assert!(scratch.push(value).is_ok());
        assert_eq!(scratch.as_slice().as_ptr(), pointer);
    }
    assert_eq!(scratch.metrics().host_allocation_events, allocation_events);
}

#[test]
fn full_scratch_reports_backpressure_without_growing() {
    let mut scratch = CommandScratch::new(1);
    assert!(scratch.push(1_u32).is_ok());
    assert!(scratch.push(2_u32).is_err());
    assert_eq!(scratch.capacity(), 1);
    assert_eq!(scratch.metrics().capacity_stalls, 1);
}
