use super::*;
use crate::fixture;

fn table() -> AtomTable {
    let structure = fixture::structure();
    let Some(table) = AtomTable::from_structure(&structure, molframe::ModelIndex::new(0)) else {
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
    assert!((radii[4] - 1.10).abs() < 1e-6, "hydrogen vdW radius");
    let colors = t.color().values();
    assert_ne!(colors[0], colors[1], "nitrogen and carbon differ");
}

#[test]
fn the_element_table_covers_every_element_the_references_define() {
    // The table is what a ligand's halogen, selenium or metal is drawn and
    // coloured from. Anything the reference engines define and this one does
    // not is a visible difference on a real structure, so the coverage is
    // asserted element by element rather than by spot check.
    let cases: [(u8, f32, [u8; 3]); 12] = [
        (1, 1.10, [0xFF, 0xFF, 0xFF]),  // H
        (6, 1.70, [0x90, 0x90, 0x90]),  // C
        (7, 1.55, [0x30, 0x50, 0xF8]),  // N
        (8, 1.52, [0xFF, 0x0D, 0x0D]),  // O
        (9, 1.47, [0x90, 0xE0, 0x50]),  // F
        (15, 1.80, [0xFF, 0x80, 0x00]), // P
        (16, 1.80, [0xFF, 0xFF, 0x30]), // S
        (17, 1.75, [0x1F, 0xF0, 0x1F]), // Cl
        (26, 2.05, [0xE0, 0x66, 0x33]), // Fe
        (30, 2.10, [0x7D, 0x80, 0xB0]), // Zn
        (34, 1.90, [0xFF, 0xA1, 0x00]), // Se
        (53, 1.98, [0x94, 0x00, 0x94]), // I
    ];
    for (z, radius, rgb) in cases {
        assert!(
            (radii::vdw_radius(z) - radius).abs() < 1e-6,
            "element {z} radius"
        );
        let color = radii::cpk_color(z);
        assert_eq!([color.r, color.g, color.b], rgb, "element {z} color");
        assert_eq!(color.a, 255, "an element colour is opaque");
    }
    // An element the table stops before takes the carbon radius rather than a
    // zero, so an exotic atom is never drawn as a point.
    assert!((radii::vdw_radius(200) - 1.70).abs() < 1e-6);
    assert!((radii::vdw_radius(0) - radii::vdw_radius(200)).abs() < 1e-6);
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
    let Some(t) = AtomTable::from_structure(&structure, molframe::ModelIndex::new(0)) else {
        panic!("fixture materializes")
    };
    assert_eq!(
        t.coords().slice().as_ptr(),
        structure.coordinates().as_ptr()
    );
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
