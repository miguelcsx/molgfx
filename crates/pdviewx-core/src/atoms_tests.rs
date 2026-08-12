use super::*;
use crate::fixture;

fn table() -> AtomTable {
    let structure = fixture::structure();
    let Some(table) = AtomTable::from_structure(&structure, pdbiox::ModelIndex::new(0)) else {
        panic!("fixture materializes")
    };
    table
}

#[test]
fn every_fixture_atom_gets_a_row_including_hetatms() {
    let t = table();
    assert_eq!(t.len(), 8, "glycine + water + sulfate rows all survive");
}

#[test]
fn elements_are_atomic_numbers_and_drive_radius_and_color() {
    let t = table();
    let elements = t.element().values();
    // Fixture order: N, C, C, O, H, O, O(water), S(sulfate).
    assert_eq!(elements[0], 7);
    assert_eq!(elements[1], 6);
    assert_eq!(elements[4], 1);
    assert_eq!(elements[7], 16);
    let radii = t.radius().values();
    assert!((radii[0] - 1.55).abs() < 1e-6, "nitrogen vdW radius");
    assert!((radii[4] - 1.20).abs() < 1e-6, "hydrogen vdW radius");
    let colors = t.color().values();
    assert_ne!(colors[0], colors[1], "nitrogen and carbon differ");
}

#[test]
fn residue_membership_follows_the_hierarchy() {
    let t = table();
    let residues = t.residue().values();
    assert_eq!(
        residues[0], residues[5],
        "all glycine atoms share a residue"
    );
    assert_ne!(residues[0], residues[6], "water is its own residue");
    assert_ne!(residues[6], residues[7], "sulfate is its own residue");
}

#[test]
fn coordinates_stay_borrowed_while_columns_are_owned() {
    let structure = fixture::structure();
    let Some(t) = AtomTable::from_structure(&structure, pdbiox::ModelIndex::new(0)) else {
        panic!("fixture materializes")
    };
    assert_eq!(t.coords().slice().as_ptr(), structure.positions().as_ptr());
}

#[test]
fn editing_flags_bumps_only_the_flags_revision() {
    let mut t = table();
    let color_before = t.color().revision();
    let flags_before = t.flags().revision();
    t.flags_mut()[0] = AtomFlags::VISIBLE.union(AtomFlags::FOCUSED);
    assert_eq!(t.color().revision(), color_before);
    assert!(t.flags().revision() > flags_before);
}

#[test]
fn all_atoms_start_visible() {
    let t = table();
    assert!(
        t.flags()
            .values()
            .iter()
            .all(|f| f.contains(AtomFlags::VISIBLE))
    );
}
