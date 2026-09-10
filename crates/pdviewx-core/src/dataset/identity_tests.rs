use super::*;

#[test]
fn global_identities_preserve_values_above_u32() {
    let value = u64::from(u32::MAX) + 17;
    assert_eq!(DatasetId::new(value).get(), value);
    assert_eq!(ChunkId::new(value).get(), value);
    assert_eq!(LogicalRow::new(value).get(), value);
}

#[test]
fn local_rows_are_explicitly_u32_sized() {
    assert_eq!(LocalRow::new(u32::MAX).get(), u32::MAX);
    assert_eq!(core::mem::size_of::<LocalRow>(), 4);
}
