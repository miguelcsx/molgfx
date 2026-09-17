use super::{SourceNamespace, SourceRows};
use std::sync::Arc;

#[test]
fn keyed_source_rows_preserve_shared_identity_and_reject_duplicates() {
    let keys: Arc<[u64]> = Arc::from([41, 57, 99]);
    let rows = SourceRows::keyed(SourceNamespace(7), Arc::clone(&keys)).unwrap();

    assert!(Arc::ptr_eq(rows.keys().unwrap(), &keys));
    assert_eq!(rows.key(1), Some(57));
    assert_eq!(rows.key(3), None);
    assert!(SourceRows::keyed(SourceNamespace(7), Arc::from([1, 1])).is_err());
}

#[test]
fn ordered_source_rows_need_no_key_allocation() {
    let rows = SourceRows::ordered(SourceNamespace(11), 4);
    assert!(rows.keys().is_none());
    assert_eq!(rows.key(3), Some(3));
}
