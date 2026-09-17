use super::*;

#[test]
fn reading_a_column_never_changes_its_revision() {
    let column = Column::new(vec![1u32, 2, 3]);
    let before = column.revision();
    let _ = column.values();
    let _ = column.len();
    assert_eq!(column.revision(), before);
}

#[test]
fn every_mutable_access_bumps_the_revision_once() {
    let mut column = Column::new(vec![1u32, 2, 3]);
    let r0 = column.revision();
    column.values_mut()[0] = 9;
    let r1 = column.revision();
    assert!(r1 > r0);
    column.values_mut()[1] = 8;
    assert!(column.revision() > r1);
}

#[test]
fn pod_columns_expose_their_bytes_for_upload() {
    let column = Column::new(vec![1.0f32, 2.0]);
    assert_eq!(column.as_bytes().len(), 8);
}
