use super::{StructureSize, auto_representations};
use crate::StructureId;
use molgfx_core::MolecularSource;
use std::fmt::Write as _;

#[test]
fn residue_counts_map_to_the_four_size_classes() {
    assert_eq!(
        StructureSize::from_polymer_residues(0),
        StructureSize::Small
    );
    assert_eq!(
        StructureSize::from_polymer_residues(9),
        StructureSize::Small
    );
    assert_eq!(
        StructureSize::from_polymer_residues(10),
        StructureSize::Medium
    );
    assert_eq!(
        StructureSize::from_polymer_residues(4_999),
        StructureSize::Medium
    );
    assert_eq!(
        StructureSize::from_polymer_residues(5_000),
        StructureSize::Large
    );
    assert_eq!(
        StructureSize::from_polymer_residues(29_999),
        StructureSize::Large
    );
    assert_eq!(
        StructureSize::from_polymer_residues(30_000),
        StructureSize::Huge
    );
}

#[test]
fn size_classes_are_ordered_by_cost() {
    assert!(StructureSize::Small < StructureSize::Medium);
    assert!(StructureSize::Large < StructureSize::Huge);
}

fn source_of(pdb: &str) -> MolecularSource {
    let result = molframe::read_bytes(
        pdb.as_bytes().to_vec(),
        Some("fixture.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("fixture must parse")
    };
    MolecularSource::from_molframe(&structure)
}

fn ligand_source() -> MolecularSource {
    source_of(
        "HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00  0.00           C\n\
HETATM    2  O1  LIG A   1       1.200   0.000   0.000  1.00  0.00           O\nEND\n",
    )
}

#[test]
fn a_ligand_without_polymer_is_drawn_at_atomic_detail() {
    let source = ligand_source();
    let Ok(forms) = auto_representations(&source, StructureId(1)) else {
        panic!("the ligand selects");
    };
    assert_eq!(forms.len(), 1);
    assert!(forms[0].explain().to_lowercase().contains("ball"));
}

/// A straight poly-alanine backbone of `residues` residues plus one ligand.
fn peptide_with_ligand(residues: u32) -> MolecularSource {
    let mut pdb = String::new();
    let mut serial = 0_u32;
    for residue in 0..residues {
        let base = f64::from(residue) * 3.8;
        for (name, element, offset) in [
            ("N", "N", 0.0),
            ("CA", "C", 1.46),
            ("C", "C", 2.5),
            ("O", "O", 3.0),
        ] {
            serial += 1;
            let _ = writeln!(
                pdb,
                "ATOM  {serial:>5}  {name:<3} ALA A{:>4}    {:>8.3}{:>8.3}{:>8.3}  1.00  0.00          {element:>2}",
                residue + 1,
                base + offset,
                0.0,
                0.0,
            );
        }
    }
    serial += 1;
    let _ = write!(
        pdb,
        "HETATM{serial:>5}  C1  LIG B 900    {:>8.3}{:>8.3}{:>8.3}  1.00  0.00           C\nEND\n",
        0.0, 20.0, 0.0
    );
    source_of(&pdb)
}

#[test]
fn a_protein_with_a_ligand_gets_a_cartoon_and_atomic_detail_for_the_rest() {
    let source = peptide_with_ligand(12);
    let Ok(forms) = auto_representations(&source, StructureId(1)) else {
        panic!("the peptide selects");
    };
    let kinds: Vec<String> = forms
        .iter()
        .map(|form| form.explain().to_lowercase())
        .collect();
    assert_eq!(forms.len(), 2, "{kinds:?}");
    assert!(kinds[0].contains("cartoon"), "{kinds:?}");
    assert!(kinds[1].contains("ball"), "{kinds:?}");
}
