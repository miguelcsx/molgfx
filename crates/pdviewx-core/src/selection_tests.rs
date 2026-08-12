use super::*;
use proptest::prelude::*;

const TABLE: u32 = 200;

fn collect(sel: &AtomSelection) -> Vec<u32> {
    let mut out = Vec::new();
    sel.for_each(TABLE, |i| out.push(i));
    out
}

fn sorted_unique(indices: Vec<u32>) -> Vec<u32> {
    let mut v: Vec<u32> = indices.into_iter().filter(|&i| i < TABLE).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Builds every encoding of the same logical index set.
fn encodings(indices: &[u32]) -> Vec<AtomSelection> {
    let sparse = AtomSelection::Sparse(indices.to_vec());
    let mut dense_bits = pdbiox::BitVec::repeat(false, TABLE);
    for &i in indices {
        dense_bits.set(i, true);
    }
    let dense = AtomSelection::Dense(dense_bits);
    let roaring = AtomSelection::Roaring(indices.iter().copied().collect());
    vec![sparse, dense, roaring]
}

proptest! {
    #[test]
    fn every_encoding_of_one_logical_set_resolves_identically(
        raw in prop::collection::vec(0u32..TABLE, 0..64),
    ) {
        let indices = sorted_unique(raw);
        let reference = indices.clone();
        for encoding in encodings(&indices) {
            prop_assert_eq!(collect(&encoding), reference.clone());
            prop_assert_eq!(encoding.count(TABLE), reference.len() as u64);
        }
    }

    #[test]
    fn union_and_intersection_are_order_independent(
        a in prop::collection::vec(0u32..TABLE, 0..48),
        b in prop::collection::vec(0u32..TABLE, 0..48),
    ) {
        let a = AtomSelection::Sparse(sorted_unique(a));
        let b = AtomSelection::Roaring(sorted_unique(b).into_iter().collect());
        prop_assert_eq!(
            collect(&a.union(&b, TABLE)),
            collect(&b.union(&a, TABLE))
        );
        prop_assert_eq!(
            collect(&a.intersect(&b, TABLE)),
            collect(&b.intersect(&a, TABLE))
        );
    }

    #[test]
    fn intersection_narrows_and_union_widens(
        a in prop::collection::vec(0u32..TABLE, 0..48),
        b in prop::collection::vec(0u32..TABLE, 0..48),
    ) {
        let sa = AtomSelection::Sparse(sorted_unique(a));
        let sb = AtomSelection::Sparse(sorted_unique(b));
        let both = sa.intersect(&sb, TABLE);
        let either = sa.union(&sb, TABLE);
        for i in collect(&both) {
            prop_assert!(sa.contains(i) && sb.contains(i));
        }
        for i in collect(&sa) {
            prop_assert!(either.contains(i));
        }
    }
}

#[test]
fn the_identities_hold_for_empty_and_all() {
    let some = AtomSelection::Range(5..25);
    assert_eq!(
        collect(&some.union(&AtomSelection::Empty, TABLE)),
        collect(&some)
    );
    assert_eq!(
        collect(&some.intersect(&AtomSelection::All, TABLE)),
        collect(&some)
    );
    assert!(collect(&some.intersect(&AtomSelection::Empty, TABLE)).is_empty());
    assert_eq!(some.difference(&AtomSelection::All, TABLE).count(TABLE), 0);
}

#[test]
fn ranges_iterate_in_ascending_order_and_clamp_to_the_table() {
    let sel = AtomSelection::Ranges(smallvec::smallvec![10..14, 190..250]);
    let indices = collect(&sel);
    assert_eq!(
        indices,
        vec![
            10, 11, 12, 13, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199
        ]
    );
}
