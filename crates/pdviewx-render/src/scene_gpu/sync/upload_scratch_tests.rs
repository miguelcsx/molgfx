use super::*;

#[test]
fn oversized_upload_arenas_are_not_retained_after_submission() {
    let mut values = Vec::<u64>::with_capacity(RETAINED_BYTES / 8 + 1);
    values.push(1);
    release(&mut values);
    assert_eq!(values.capacity(), 0);
}

#[test]
fn small_upload_arenas_reuse_their_allocation() {
    let mut values = Vec::<u64>::with_capacity(64);
    values.push(1);
    release(&mut values);
    assert_eq!(values.capacity(), 64);
    assert!(values.is_empty());
}
