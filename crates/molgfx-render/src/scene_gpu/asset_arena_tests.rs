use super::{AssetArena, AssetArenaError, STORAGE_ALIGNMENT};
use crate::testing::MockDevice;

#[test]
fn allocations_are_aligned_non_overlapping_ranges_in_one_buffer() {
    let device = MockDevice::default();
    let queue = device.queue();
    let Ok(mut arena) = AssetArena::with_initial_bytes(&device, 1024) else {
        panic!("small arena is valid")
    };
    let Ok(first) = arena.upload(&device, &queue, &[1; 17]) else {
        panic!("first payload fits")
    };
    let Ok(second) = arena.upload(&device, &queue, &[2; 300]) else {
        panic!("second payload fits")
    };
    assert_eq!(first.offset() % STORAGE_ALIGNMENT, 0);
    assert_eq!(second.offset() % STORAGE_ALIGNMENT, 0);
    assert!(first.offset() + first.allocated_bytes() <= second.offset());
    assert_eq!(arena.metrics().physical_buffers, 1);
}

#[test]
fn growth_relocates_once_and_preserves_stable_offsets() {
    let device = MockDevice::default();
    let queue = device.queue();
    let Ok(mut arena) = AssetArena::with_initial_bytes(&device, 256) else {
        panic!("small arena is valid")
    };
    let Ok(first) = arena.upload(&device, &queue, &[3; 64]) else {
        panic!("first payload fits")
    };
    let initial_revision = arena.revision();
    let Ok(second) = arena.upload(&device, &queue, &[4; 300]) else {
        panic!("growth admits second payload")
    };
    assert_eq!(first.offset(), 0);
    assert!(second.offset() >= STORAGE_ALIGNMENT);
    assert!(arena.revision() > initial_revision);
    assert_eq!(arena.metrics().relocations, 1);
    assert_eq!(arena.metrics().physical_buffers, 1);
    let copies = match device.log.buffer_copies.lock() {
        Ok(copies) => copies,
        Err(error) => panic!("copy log locks: {error}"),
    };
    assert_eq!(copies.len(), 1);
    assert_eq!(copies[0].2, STORAGE_ALIGNMENT);
}

#[test]
fn device_capacity_returns_typed_backpressure() {
    let device = MockDevice::with_storage_limit(512);
    let queue = device.queue();
    let Ok(mut arena) = AssetArena::with_initial_bytes(&device, 256) else {
        panic!("limited arena is valid")
    };
    let result = arena.upload(&device, &queue, &[5; 768]);
    assert!(matches!(result, Err(AssetArenaError::Backpressure { .. })));
}

#[test]
fn release_reclaims_physical_residency_without_destroying_the_buffer() {
    let device = MockDevice::default();
    let queue = device.queue();
    let Ok(mut arena) = AssetArena::with_initial_bytes(&device, 512) else {
        panic!("small arena is valid")
    };
    let Ok(range) = arena.upload(&device, &queue, &[6; 64]) else {
        panic!("payload fits")
    };
    assert!(arena.metrics().resident_bytes > 0);
    assert!(arena.release(range).is_ok());
    assert_eq!(arena.metrics().resident_bytes, 0);
    assert_eq!(arena.metrics().physical_buffers, 1);
}
