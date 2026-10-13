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

fn indexed(index: usize) -> Selection {
    Selection::from(format!("index {index}"))
}

#[test]
fn the_cache_is_bounded_by_bytes() {
    let source = source();
    let one = SelectionRows::with_budget(usize::MAX);
    assert!(one.rows(StructureId(1), &source, &indexed(0)).is_ok());
    let cost = one.bytes();
    let cache = SelectionRows::with_budget(cost * 4);
    for index in 0..40 {
        assert!(cache.rows(StructureId(1), &source, &indexed(index)).is_ok());
    }
    assert!(cache.bytes() <= cost * 4);
    assert!(cache.len() <= 4 && cache.len() >= 3, "{}", cache.len());
}

#[test]
fn evicts_the_least_recently_used_and_keeps_the_touched_one() {
    let source = source();
    let probe = SelectionRows::with_budget(usize::MAX);
    assert!(probe.rows(StructureId(1), &source, &indexed(0)).is_ok());
    let cache = SelectionRows::with_budget(probe.bytes() * 3);
    let Ok((first, _)) = cache.rows(StructureId(1), &source, &indexed(0)) else {
        panic!("evaluates")
    };
    for index in 1..3 {
        assert!(cache.rows(StructureId(1), &source, &indexed(index)).is_ok());
    }
    // Touch the oldest, then overflow: entry 1 is now the oldest.
    assert!(cache.rows(StructureId(1), &source, &indexed(0)).is_ok());
    assert!(cache.rows(StructureId(1), &source, &indexed(3)).is_ok());
    let Ok((again, _)) = cache.rows(StructureId(1), &source, &indexed(0)) else {
        panic!("evaluates")
    };
    assert!(Arc::ptr_eq(&first, &again), "the touched entry survived");
}

#[test]
fn a_working_set_past_the_old_entry_cap_is_fully_retained() {
    let source = source();
    let cache = SelectionRows::default();
    let count = 2_000;
    let mut first_pass = Vec::with_capacity(count);
    for index in 0..count {
        let Ok((rows, _)) = cache.rows(StructureId(1), &source, &indexed(index)) else {
            panic!("evaluates")
        };
        first_pass.push(rows);
    }
    assert_eq!(cache.len(), count);
    for (index, expected) in first_pass.iter().enumerate() {
        let Ok((rows, _)) = cache.rows(StructureId(1), &source, &indexed(index)) else {
            panic!("evaluates")
        };
        assert!(
            Arc::ptr_eq(expected, &rows),
            "entry {index} was re-evaluated"
        );
    }
}

#[test]
fn forgetting_a_source_releases_its_bytes() {
    let source = source();
    let cache = SelectionRows::default();
    for index in 0..8 {
        assert!(cache.rows(StructureId(1), &source, &indexed(index)).is_ok());
    }
    cache.forget(source.identity());
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.bytes(), 0);
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

#[test]
fn placed_copies_of_one_source_share_one_evaluation() {
    let cache = SelectionRows::default();
    let source = source();
    let selection = Selection::from("all");
    let (Ok((first, _)), Ok((second, _))) = (
        cache.rows(StructureId(1), &source, &selection),
        cache.rows(StructureId(7), &source.clone(), &selection),
    ) else {
        panic!("the query evaluates")
    };
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(cache.len(), 1);
}
