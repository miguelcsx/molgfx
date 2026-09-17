use super::{EntitySelection, EntitySelectionRows, GpuEntitySelection};
use crate::{PointBatchHandle, RowDomain, handle::RawHandle};

fn domain() -> RowDomain {
    RowDomain::Points(PointBatchHandle(RawHandle::new_for_test(2, 3)))
}

#[test]
fn ranges_and_sparse_rows_are_chosen_from_canonical_runs() {
    let ranges = EntitySelection::from_rows(domain(), 100, (10..40).collect()).unwrap();
    assert!(matches!(ranges.rows(), EntitySelectionRows::Ranges(_)));

    let sparse = EntitySelection::from_rows(domain(), 100, vec![1, 10, 90]).unwrap();
    assert!(matches!(sparse.rows(), EntitySelectionRows::Sparse(_)));
}

#[test]
fn gpu_encoding_changes_only_with_density() {
    let dense = EntitySelection::from_rows(domain(), 64, (0..32).collect()).unwrap();
    let sparse = EntitySelection::from_rows(domain(), 1_000, vec![2, 20, 200]).unwrap();
    assert!(matches!(
        dense.gpu_encoding(),
        GpuEntitySelection::Bitset(_)
    ));
    assert!(matches!(
        sparse.gpu_encoding(),
        GpuEntitySelection::Compact(_)
    ));
}
