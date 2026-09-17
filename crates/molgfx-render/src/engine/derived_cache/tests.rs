use super::{
    DerivedCache, DerivedCacheBudget, DerivedCacheClass, DerivedCacheKey, DerivedCacheUsage,
    DerivedFootprint, MaterializationPlan,
};

const TEN_GPU: DerivedFootprint = DerivedFootprint {
    cpu_bytes: 0,
    gpu_bytes: 10,
};

#[test]
fn appearance_is_evicted_before_static_endpoints() {
    let mut cache = DerivedCache::new(DerivedCacheBudget {
        cpu_bytes: 0,
        gpu_bytes: 20,
    });
    assert!(cache.retain(20, DerivedCacheClass::StaticEndpoints, TEN_GPU, 1));
    assert!(cache.retain(10, DerivedCacheClass::Appearance, TEN_GPU, 2));
    assert!(cache.retain(30, DerivedCacheClass::StaticEndpoints, TEN_GPU, 3));

    assert!(cache.contains(key(20)));
    assert!(cache.contains(key(30)));
    assert!(!cache.contains(key(10)));
}

#[test]
fn planner_materializes_only_reused_results_that_fit() {
    let mut cache = DerivedCache::new(DerivedCacheBudget {
        cpu_bytes: 0,
        gpu_bytes: 10,
    });
    assert_eq!(cache.plan(1, TEN_GPU, 1, 0), MaterializationPlan::Direct);
    assert_eq!(
        cache.plan(1, TEN_GPU, 2, 0),
        MaterializationPlan::Materialized
    );
    assert_eq!(
        cache.plan(2, TEN_GPU, 2, 1),
        MaterializationPlan::Materialized
    );
    assert_eq!(cache.take_evictions().collect::<Vec<_>>(), vec![key(1)]);
}

#[test]
fn frame_then_stable_key_decide_eviction_not_insertion_order() {
    let mut cache = DerivedCache::new(DerivedCacheBudget {
        cpu_bytes: 0,
        gpu_bytes: 20,
    });
    assert!(cache.retain(9, DerivedCacheClass::Appearance, TEN_GPU, 4));
    assert!(cache.retain(3, DerivedCacheClass::Appearance, TEN_GPU, 4));
    assert!(cache.retain(7, DerivedCacheClass::Appearance, TEN_GPU, 5));

    assert!(!cache.contains(key(3)));
    assert!(cache.contains(key(7)));
    assert!(cache.contains(key(9)));
}

const fn key(value: u64) -> DerivedCacheKey {
    DerivedCacheKey::Test(value)
}

#[test]
fn impossible_item_is_rejected_without_exceeding_or_changing_peak() {
    let mut cache = DerivedCache::new(DerivedCacheBudget {
        cpu_bytes: 4,
        gpu_bytes: 4,
    });
    assert!(!cache.retain(
        1,
        DerivedCacheClass::Appearance,
        DerivedFootprint {
            cpu_bytes: 5,
            gpu_bytes: 0,
        },
        0,
    ));
    assert_eq!(cache.usage(), DerivedCacheUsage::default());
}

#[test]
fn full_width_occurrence_keys_never_alias() {
    let mut cache = DerivedCache::new(DerivedCacheBudget {
        cpu_bytes: 0,
        gpu_bytes: 20,
    });
    let first =
        DerivedCacheKey::PagedInstance(molgfx_core::ChunkOccurrenceId::new(0x0100_0000_0000_0001));
    let second =
        DerivedCacheKey::PagedInstance(molgfx_core::ChunkOccurrenceId::new(0x0200_0000_0000_0001));
    assert!(cache.retain(
        first,
        DerivedCacheClass::TimelineMaterialization,
        TEN_GPU,
        1
    ));
    assert!(cache.retain(
        second,
        DerivedCacheClass::TimelineMaterialization,
        TEN_GPU,
        1
    ));
    assert!(cache.contains(first));
    assert!(cache.contains(second));
}
