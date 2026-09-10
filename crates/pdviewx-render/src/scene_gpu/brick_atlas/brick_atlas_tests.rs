use super::types::{BrickAtlasConfig, BrickAtlasError, BrickAtlasKind, BrickAtlasUpload};
use super::upload::GpuBrickAtlas;
use crate::testing::MockDevice;
use pdviewx_core::{
    BrickAddress, BrickCatalog, BrickDescriptor, BrickId, BrickMetadata, BrickShape,
    BrickValueRange, ChunkId, DatasetId, DirtyGeneration,
};
use pdviewx_gpu::{UploadBackpressure, UploadRingConfig};
use std::sync::atomic::Ordering;

fn descriptor(id: u64, origin_x: u64, generation: u64) -> BrickDescriptor {
    let Ok(shape) = BrickShape::new([4, 4, 4], 1) else {
        panic!("fixture shape must be valid");
    };
    let Ok(metadata) = BrickMetadata::new(
        BrickId::new(id),
        BrickAddress {
            origin: [origin_x, 0, 0],
            mip: 0,
        },
        shape,
        BrickValueRange::Scalar { min: 0.0, max: 1.0 },
        DirtyGeneration::new(generation),
    ) else {
        panic!("fixture metadata must be valid");
    };
    BrickDescriptor {
        chunk: ChunkId::new(id),
        metadata,
    }
}

fn catalog(rows: &[BrickDescriptor]) -> BrickCatalog {
    let Ok(catalog) = BrickCatalog::new(DatasetId::new(9), [1 << 40, 8, 8], 4, rows.to_vec())
    else {
        panic!("fixture catalog must be valid");
    };
    catalog
}

fn config(capacity: usize) -> BrickAtlasConfig {
    BrickAtlasConfig {
        stored_shape: [4, 4, 4],
        resident_capacity: capacity,
        uploads: UploadRingConfig {
            capacity_bytes: capacity.saturating_mul(256).max(256),
            ticket_capacity: capacity.saturating_mul(2).max(2),
            epoch_budget_bytes: capacity.saturating_mul(256).max(256),
            in_flight_budget_bytes: capacity.saturating_mul(256).max(256),
            alignment: 4,
        },
        kind: BrickAtlasKind::Scalar,
    }
}

#[test]
fn adjacent_bricks_upload_complete_halos_and_resolve_to_distinct_slots() {
    let rows = [descriptor(1, 0, 1), descriptor(2, 2, 1)];
    let catalog = catalog(&rows);
    let device = MockDevice::default();
    let queue = device.queue();
    let Ok(mut atlas) = GpuBrickAtlas::new(&device, &queue, &catalog, config(2)) else {
        panic!("atlas must fit the mock device");
    };
    let first = vec![1u8; 256];
    let second = vec![2u8; 256];
    assert!(
        atlas
            .stage(
                &device,
                &queue,
                BrickAtlasUpload {
                    descriptor: rows[0],
                    bytes: &first
                }
            )
            .is_ok()
    );
    assert!(
        atlas
            .stage(
                &device,
                &queue,
                BrickAtlasUpload {
                    descriptor: rows[1],
                    bytes: &second
                }
            )
            .is_ok()
    );
    device.complete_submissions();
    let Ok(poll) = atlas.poll(&device, &queue) else {
        panic!("completed uploads must publish");
    };
    assert_eq!(poll.uploads_published, 2);
    assert_ne!(
        atlas.resolve(rows[0].metadata.address),
        atlas.resolve(rows[1].metadata.address)
    );
    let Ok(writes) = device.log.texture_writes.lock() else {
        panic!("mock texture log must be readable");
    };
    assert_eq!(
        writes
            .iter()
            .filter(|entry| entry.0 == "sparse scalar brick atlas")
            .count(),
        2
    );
    assert!(
        writes
            .iter()
            .filter(|entry| entry.0 == "sparse scalar brick atlas")
            .all(|entry| entry.1 == 256)
    );
}

#[test]
fn stale_completion_never_replaces_the_newer_generation() {
    let old = descriptor(3, 0, 1);
    let current = descriptor(3, 0, 2);
    let catalog = catalog(&[current]);
    let device = MockDevice::default();
    let queue = device.queue();
    let mut stale_config = config(1);
    stale_config.uploads.capacity_bytes = 512;
    stale_config.uploads.epoch_budget_bytes = 512;
    stale_config.uploads.in_flight_budget_bytes = 512;
    let Ok(mut atlas) = GpuBrickAtlas::new(&device, &queue, &catalog, stale_config) else {
        panic!("atlas must fit");
    };
    let bytes = vec![0u8; 256];
    assert!(
        atlas
            .stage(
                &device,
                &queue,
                BrickAtlasUpload {
                    descriptor: old,
                    bytes: &bytes
                }
            )
            .is_ok()
    );
    assert!(
        atlas
            .stage(
                &device,
                &queue,
                BrickAtlasUpload {
                    descriptor: current,
                    bytes: &bytes
                }
            )
            .is_ok()
    );
    device.log.completed_fence.store(1, Ordering::Release);
    let Ok(first) = atlas.poll(&device, &queue) else {
        panic!("first fence must poll");
    };
    assert_eq!(first.stale_completions, 1);
    assert_eq!(atlas.resolve(current.metadata.address), None);
    device.complete_submissions();
    let Ok(second) = atlas.poll(&device, &queue) else {
        panic!("current fence must poll");
    };
    assert_eq!(second.uploads_published, 1);
    assert_eq!(atlas.resolve(current.metadata.address), Some(0));
}

#[test]
fn eviction_holds_the_slot_until_its_fence_completes() {
    let first = descriptor(4, 0, 1);
    let next = descriptor(5, 2, 1);
    let catalog = catalog(&[first, next]);
    let device = MockDevice::default();
    let queue = device.queue();
    let Ok(mut atlas) = GpuBrickAtlas::new(&device, &queue, &catalog, config(1)) else {
        panic!("atlas must fit");
    };
    let bytes = vec![0u8; 256];
    assert!(
        atlas
            .stage(
                &device,
                &queue,
                BrickAtlasUpload {
                    descriptor: first,
                    bytes: &bytes
                }
            )
            .is_ok()
    );
    device.complete_submissions();
    assert!(atlas.poll(&device, &queue).is_ok());
    assert!(
        atlas
            .request_eviction(&device, &queue, first.metadata.id)
            .is_ok()
    );
    assert_eq!(atlas.resolve(first.metadata.address), None);
    atlas.begin_frame();
    assert!(matches!(
        atlas.stage(
            &device,
            &queue,
            BrickAtlasUpload {
                descriptor: next,
                bytes: &bytes
            }
        ),
        Err(BrickAtlasError::WorkingSet(_))
    ));
    device.complete_submissions();
    assert!(atlas.poll(&device, &queue).is_ok());
    atlas.begin_frame();
    assert!(
        atlas
            .stage(
                &device,
                &queue,
                BrickAtlasUpload {
                    descriptor: next,
                    bytes: &bytes
                }
            )
            .is_ok()
    );
}

#[test]
fn fixed_budgets_bound_memory_and_apply_typed_backpressure() {
    let rows = [descriptor(6, 0, 1), descriptor(7, 2, 1)];
    let catalog = catalog(&rows);
    let device = MockDevice::default();
    let queue = device.queue();
    let mut limited = config(2);
    limited.uploads.in_flight_budget_bytes = 256;
    let Ok(mut atlas) = GpuBrickAtlas::new(&device, &queue, &catalog, limited) else {
        panic!("atlas must fit");
    };
    let bytes = vec![0u8; 256];
    assert!(
        atlas
            .stage(
                &device,
                &queue,
                BrickAtlasUpload {
                    descriptor: rows[0],
                    bytes: &bytes
                }
            )
            .is_ok()
    );
    assert!(matches!(
        atlas.stage(
            &device,
            &queue,
            BrickAtlasUpload {
                descriptor: rows[1],
                bytes: &bytes
            }
        ),
        Err(BrickAtlasError::Upload(UploadBackpressure::InFlightBudget))
    ));
    let before = atlas.metrics();
    let Ok(writes_before) = device.log.writes.lock().map(|writes| writes.len()) else {
        panic!("mock writes must be readable");
    };
    atlas.begin_frame();
    assert!(atlas.poll(&device, &queue).is_ok());
    let Ok(writes_after) = device.log.writes.lock().map(|writes| writes.len()) else {
        panic!("mock writes must be readable");
    };
    assert_eq!(writes_before, writes_after);
    assert_eq!(atlas.metrics().atlas_bytes, 512);
    assert_eq!(atlas.metrics().page_table_bytes, before.page_table_bytes);
}
