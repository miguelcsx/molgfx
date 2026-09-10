use super::GpuAssetIdentity;
use pdviewx_core::DatasetId;

#[test]
fn generations_are_distinct_but_equal_generations_reject_fingerprint_collisions() {
    let resident = GpuAssetIdentity {
        dataset: DatasetId::new(9),
        coordinate_generation: 3,
        fingerprint: 11,
    };
    let newer = GpuAssetIdentity {
        coordinate_generation: 4,
        fingerprint: 12,
        ..resident
    };
    let collision = GpuAssetIdentity {
        fingerprint: 13,
        ..resident
    };
    assert!(!resident.conflicts_with(newer));
    assert!(resident.conflicts_with(collision));
}
