use super::*;

#[test]
fn large_upload_staging_is_not_retained() {
    let records = RETAINED_STAGING_BYTES / std::mem::size_of::<PrimitiveGpu>() + 1;
    let mut values = Vec::<PrimitiveGpu>::with_capacity(records);

    release_staging(&mut values);

    assert_eq!(values.capacity(), 0);
}

#[test]
fn small_upload_staging_keeps_its_allocation() {
    let mut values = Vec::<PrimitiveGpu>::with_capacity(32);
    let capacity = values.capacity();

    release_staging(&mut values);

    assert_eq!(values.capacity(), capacity);
}

#[test]
fn realtime_shadow_limit_counts_only_opaque_primitive_rows() {
    assert!(shadows_enabled(REALTIME_SHADOW_PRIMITIVES, false));
    assert!(!shadows_enabled(REALTIME_SHADOW_PRIMITIVES + 1, false));
    assert!(shadows_enabled(REALTIME_SHADOW_PRIMITIVES + 1, true));
    assert!(!shadows_enabled(0, true));
}
