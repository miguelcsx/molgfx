use super::*;
use crate::fixture;

fn hierarchy() -> Hierarchy {
    Hierarchy::from_structure(&fixture::structure())
}

#[test]
fn the_fixture_has_three_residues_in_one_chain() {
    let h = hierarchy();
    assert_eq!(h.residue_count(), 3);
    assert_eq!(h.chain_count(), 1);
    assert_eq!(h.chain_residues(0), 0..3);
}

#[test]
fn residue_atom_ranges_tile_the_atom_column_without_gaps() {
    let h = hierarchy();
    let mut expected_start = 0u32;
    for i in 0..h.residue_count() {
        let range = h.residue_atoms(i);
        assert_eq!(range.start, expected_start);
        assert!(range.end > range.start, "no empty residues in the fixture");
        expected_start = range.end;
    }
    assert_eq!(expected_start, 8, "ranges cover all eight atoms");
}

#[test]
fn membership_lookups_invert_the_ranges() {
    let h = hierarchy();
    for i in 0..h.residue_count() {
        let range = h.residue_atoms(i);
        for atom in range {
            assert_eq!(h.residue_of_atom(atom), Some(i));
        }
    }
    assert_eq!(h.residue_of_atom(999), None);
    assert_eq!(h.chain_of_residue(0), Some(0));
    assert_eq!(h.chain_of_residue(99), None);
}

#[test]
fn a_query_past_the_tables_returns_an_empty_range() {
    let h = hierarchy();
    assert!(h.residue_atoms(99).is_empty());
    assert!(h.chain_residues(99).is_empty());
}
