use super::distance::pack_distance_bonds;
use molgfx_core::{
    AtomSelection, PlacedStructure, Representation, RepresentationKind, RepresentationTarget, Scene,
};

#[test]
fn distance_links_keep_shared_atoms_but_never_join_mutually_exclusive_conformers() {
    let pdb = b"HETATM    1  C1 AUNK A   1       0.000   0.000   0.000  0.50 10.00           C\nHETATM    2  C2 BUNK A   1       1.000   0.000   0.000  0.50 10.00           C\nHETATM    3  C3  UNK A   1       0.000   1.000   0.000  1.00 10.00           C\nEND\n";
    let (source, _) = molframe::read_bytes(
        pdb.to_vec(),
        Some("alternate.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source");
    let placed = PlacedStructure::new(&source).expect("placed");
    let mut scene = Scene::from_structure(&source).expect("scene");
    let selection = scene.add_selection(AtomSelection::All);
    let mut representation = Representation::new(
        RepresentationTarget::Selection(selection),
        RepresentationKind::DynamicBonds,
    );
    representation.params.distance_cutoff = 1.6;
    let mut bonds = Vec::new();
    pack_distance_bonds(&placed, &representation, &[0, 1, 2], &mut bonds).expect("pairs");
    assert_eq!(
        bonds
            .iter()
            .map(|bond| (bond.atom_a, bond.atom_b))
            .collect::<Vec<_>>(),
        vec![(0, 2), (1, 2)]
    );
    assert_eq!(
        bonds.len(),
        2,
        "only the two edges to the shared blank-labelled atom coexist"
    );
}
