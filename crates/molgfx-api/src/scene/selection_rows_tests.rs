use super::*;

fn source() -> MolecularSource {
    MolecularSource::from_molframe(&super::super::tests::structure())
}

#[test]
fn an_unchanged_query_is_evaluated_once_and_shared() {
    let cache = SelectionRows::default();
    let source = source();
    let selection = Selection::from("all");
    let (Ok((first, _)), Ok((second, _))) = (
        cache.rows(StructureId(1), &source, &selection),
        cache.rows(StructureId(1), &source, &Selection::from("all")),
    ) else {
        panic!("the query evaluates")
    };
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(cache.len(), 1);
}

#[test]
fn equivalent_texts_share_rows_through_their_fingerprint() {
    let cache = SelectionRows::default();
    let source = source();
    let (Ok((first, a)), Ok((second, b))) = (
        cache.rows(StructureId(1), &source, &Selection::from("all")),
        cache.rows(StructureId(1), &source, &Selection::from("(all)")),
    ) else {
        panic!("the query evaluates")
    };
    assert_eq!(a, b);
    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn the_cache_is_bounded() {
    let cache = SelectionRows::default();
    let source = source();
    for index in 0..(SelectionRows::CAPACITY + 8) {
        let selection = Selection::from(format!("index {index}"));
        assert!(cache.rows(StructureId(1), &source, &selection).is_ok());
    }
    assert_eq!(cache.len(), SelectionRows::CAPACITY);
}

#[test]
fn a_malformed_query_is_an_error_and_not_cached() {
    let cache = SelectionRows::default();
    assert!(
        cache
            .rows(StructureId(1), &source(), &Selection::from("resname ("))
            .is_err()
    );
    assert_eq!(cache.len(), 0);
}
