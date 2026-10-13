use super::CullScratch;

fn owners(keys: &[Option<(u32, u64)>]) -> Vec<bool> {
    let mut scratch = CullScratch::<u32>::default();
    scratch.mark(keys.iter().copied());
    (0..keys.len()).map(|index| scratch.owns(index)).collect()
}

/// The reference the scratch replaces: a linear scan of groups seen so far.
fn quadratic(keys: &[Option<(u32, u64)>]) -> Vec<bool> {
    let mut seen = Vec::new();
    keys.iter()
        .map(|key| match key {
            Some(key) => {
                if seen.contains(key) {
                    false
                } else {
                    seen.push(*key);
                    true
                }
            }
            None => true,
        })
        .collect()
}

#[test]
fn the_first_slot_of_each_shared_group_owns_the_dispatch() {
    let keys = [Some((1, 0)), Some((2, 0)), Some((1, 0)), Some((1, 8)), None];
    assert_eq!(owners(&keys), [true, true, false, true, true]);
}

#[test]
fn owners_match_the_linear_scan_on_a_dense_scene() {
    let keys: Vec<_> = (0..2_000u32)
        .map(|slot| {
            if slot % 13 == 0 {
                None
            } else {
                Some((slot * 7 % 97, u64::from(slot % 3)))
            }
        })
        .collect();
    assert_eq!(owners(&keys), quadratic(&keys));
}

#[test]
fn marking_again_reuses_capacity_and_replaces_the_marks() {
    let mut scratch = CullScratch::<u32>::default();
    let keys: Vec<_> = (0..64u32).map(|slot| Some((slot % 5, 0))).collect();
    scratch.mark(keys.iter().copied());
    let capacity = (scratch.keyed.capacity(), scratch.owner.capacity());
    scratch.mark(keys.iter().copied().take(3));
    assert_eq!(
        (scratch.keyed.capacity(), scratch.owner.capacity()),
        capacity
    );
    assert!(scratch.owns(2));
    assert!(!scratch.owns(3));
}
