use super::AtomCategory;
use molgfx_core::MolecularSource;
use num_traits::ToPrimitive as _;

const TWO_CHAINS: &str = "\
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00  0.00           N\n\
ATOM      2  CA  ALA A   1       1.460   0.000   0.000  1.00  0.00           C\n\
ATOM      3  N   GLY A   2       3.800   0.000   0.000  1.00  0.00           N\n\
ATOM      4  CA  GLY A   2       5.260   0.000   0.000  1.00  0.00           C\n\
ATOM      5  N   ALA B   1       0.000   9.000   0.000  1.00  0.00           N\n\
ATOM      6  CA  ALA B   1       1.460   9.000   0.000  1.00  0.00           C\n\
HETATM    7 ZN    ZN C   1       0.000  20.000   0.000  1.00  0.00          ZN\n\
END\n";

fn source() -> MolecularSource {
    let result = molframe::read_bytes(
        TWO_CHAINS.as_bytes().to_vec(),
        Some("chains.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("fixture must parse")
    };
    MolecularSource::from_molframe(&structure)
}

/// The whole-number categories of a column, `None` where an atom has none.
fn column(category: AtomCategory) -> Vec<Option<u32>> {
    category
        .column(&source())
        .unwrap_or_else(|error| panic!("{error}"))
        .into_iter()
        .map(|value| value.to_u32())
        .collect()
}

#[test]
fn chains_are_numbered_in_file_order_for_every_atom() {
    assert_eq!(column(AtomCategory::Chain), [0, 0, 0, 0, 1, 1, 2].map(Some));
}

#[test]
fn neighbouring_residues_get_categories_that_differ_by_the_coprime_step() {
    let residues = column(AtomCategory::Residue);
    assert_eq!(residues[0], Some(0));
    assert_eq!(residues[2], Some(5));
    assert_eq!(residues[4], Some(10));
}

#[test]
fn a_residue_name_is_its_standard_kind_and_anything_else_is_missing() {
    let names = column(AtomCategory::ResidueName);
    assert_eq!(names[0], Some(0), "alanine");
    assert_eq!(names[2], Some(7), "glycine");
    assert_eq!(names[6], None, "a zinc ion is not a standard residue");
}

#[test]
fn molecule_types_separate_protein_from_ions() {
    let types = column(AtomCategory::MoleculeType);
    assert_eq!(types[0], Some(3), "protein");
    assert_eq!(types[6], Some(2), "ion");
}

#[test]
fn every_category_names_a_palette_and_round_trips_its_name() {
    for category in AtomCategory::ALL {
        assert_eq!(AtomCategory::from_name(category.name()), Some(category));
        assert!(category.default_palette().len() >= 2);
    }
}
