use super::*;

#[test]
fn a_handle_resolves_until_its_entry_is_removed() {
    let mut map: SlotMap<&'static str> = SlotMap::new();
    let h = map.insert("alpha");
    assert_eq!(map.get(h), Some(&"alpha"));
    assert_eq!(map.remove(h), Some("alpha"));
    assert_eq!(map.get(h), None, "removal staled the handle");
}

#[test]
fn a_reused_slot_does_not_resurrect_an_old_handle() {
    let mut map: SlotMap<u32> = SlotMap::new();
    let old = map.insert(1);
    map.remove(old);
    let new = map.insert(2);
    assert_eq!(
        map.get(old),
        None,
        "old generation must not see the new value"
    );
    assert_eq!(map.get(new), Some(&2));
}

#[test]
fn handles_survive_unrelated_removals() {
    let mut map: SlotMap<u32> = SlotMap::new();
    let a = map.insert(10);
    let b = map.insert(20);
    let c = map.insert(30);
    map.remove(b);
    assert_eq!(map.get(a), Some(&10));
    assert_eq!(map.get(c), Some(&30));
    assert_eq!(map.len(), 2);
}

#[test]
fn iteration_order_is_slot_order_and_stable_across_removals() {
    let mut map: SlotMap<u32> = SlotMap::new();
    let _a = map.insert(1);
    let b = map.insert(2);
    let _c = map.insert(3);
    map.remove(b);
    let values: Vec<u32> = map.iter().map(|(_, v)| *v).collect();
    assert_eq!(values, vec![1, 3]);
}

#[test]
fn manifest_preflight_rejects_a_hostile_sparse_row_before_reserving() {
    let mut map: SlotMap<u32> = SlotMap::new();
    let slot_capacity = map.slots.capacity();
    let sparse_capacity = map.sparse.capacity();
    let free_capacity = map.free.capacity();

    assert_eq!(
        map.prepare_manifest_rows([u32::MAX].into_iter()),
        Err(ManifestRowsError::RowOutOfRange)
    );
    assert_eq!(map.slots.capacity(), slot_capacity);
    assert_eq!(map.sparse.capacity(), sparse_capacity);
    assert_eq!(map.free.capacity(), free_capacity);
}

#[test]
fn manifest_preflight_preserves_reasonable_holes_and_generations() {
    let mut map: SlotMap<u32> = SlotMap::new();
    assert!(map.prepare_manifest_rows([2, 7].into_iter()).is_ok());
    let first = RawHandle::new_for_test(2, 9);
    let second = RawHandle::new_for_test(7, 4);

    assert_eq!(map.insert_at(first, 20), Some(()));
    assert_eq!(map.insert_at(second, 70), Some(()));
    assert_eq!(map.get(first), Some(&20));
    assert_eq!(map.get(second), Some(&70));
}

#[test]
fn a_high_sparse_manifest_row_costs_one_slot_and_round_trips() {
    let mut map: SlotMap<u32> = SlotMap::new();
    let row = crate::EntityId::MAX_INDEX;
    assert!(map.prepare_manifest_rows([row].into_iter()).is_ok());
    assert_eq!(map.slots.capacity(), 0);
    assert_eq!(map.sparse.capacity(), 1);
    let handle = RawHandle::new_for_test(row, 19);

    assert_eq!(map.insert_at(handle, 70), Some(()));
    assert_eq!(map.get(handle), Some(&70));
    assert_eq!(map.iter().map(|(raw, _)| raw).collect::<Vec<_>>(), [handle]);
    assert_eq!(map.len(), 1);
}

#[test]
fn normal_inserts_fill_rows_before_a_sparse_manifest_identity() {
    let mut map: SlotMap<u32> = SlotMap::new();
    let sparse = RawHandle::new_for_test(2, 7);
    assert_eq!(map.insert_at(sparse, 20), Some(()));
    let first = map.insert(0);
    let second = map.insert(10);
    let third = map.insert(30);

    assert_eq!(first.row(), 0);
    assert_eq!(second.row(), 1);
    assert_eq!(third.row(), 3);
    assert_eq!(map.get(sparse), Some(&20));
    assert_eq!(map.len(), 4);
}

#[test]
fn manifest_preflight_rejects_duplicate_or_unsorted_rows() {
    let mut map: SlotMap<u32> = SlotMap::new();
    assert_eq!(
        map.prepare_manifest_rows([3, 3].into_iter()),
        Err(ManifestRowsError::InconsistentOrder)
    );
    assert_eq!(
        map.prepare_manifest_rows([3, 2].into_iter()),
        Err(ManifestRowsError::InconsistentOrder)
    );
}
