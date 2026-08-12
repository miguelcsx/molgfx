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
