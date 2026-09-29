use super::MolecularSource;

/// Three alanines with backbone geometry close to an extended chain, a HELIX
/// record covering all of them, and one explicit CONECT record.
const TRIPEPTIDE: &str = "\
HELIX    1   1 ALA A    1  ALA A    3  1                                   3\n\
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00  0.00           N\n\
ATOM      2  CA  ALA A   1       1.458   0.000   0.000  1.00  0.00           C\n\
ATOM      3  C   ALA A   1       2.009   1.420   0.000  1.00  0.00           C\n\
ATOM      4  O   ALA A   1       1.251   2.390   0.000  1.00  0.00           O\n\
ATOM      5  N   ALA A   2       3.332   1.540   0.000  1.00  0.00           N\n\
ATOM      6  CA  ALA A   2       3.966   2.850   0.000  1.00  0.00           C\n\
ATOM      7  C   ALA A   2       5.480   2.750   0.000  1.00  0.00           C\n\
ATOM      8  O   ALA A   2       6.020   1.650   0.000  1.00  0.00           O\n\
ATOM      9  N   ALA A   3       6.150   3.890   0.000  1.00  0.00           N\n\
ATOM     10  CA  ALA A   3       7.600   3.980   0.000  1.00  0.00           C\n\
ATOM     11  C   ALA A   3       8.200   5.380   0.000  1.00  0.00           C\n\
ATOM     12  O   ALA A   3       7.500   6.380   0.000  1.00  0.00           O\n\
CONECT    1    2\n\
END\n";

fn source() -> MolecularSource {
    let result = molframe::read_bytes(
        TRIPEPTIDE.as_bytes().to_vec(),
        Some("tripeptide.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("fixture must parse")
    };
    MolecularSource::from_molframe(&structure)
}

/// One alanine whose CA carries an anisotropic displacement tensor, so the
/// molframe anisotropy table is exercised on a real read.
const ANISO_PDB: &str = "\
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00  0.00           N\n\
ATOM      2  CA  ALA A   1       1.458   0.000   0.000  1.00  0.00           C\n\
ANISOU    2  CA  ALA A   1   100000 200000 300000  40000  50000  60000       C\n\
ATOM      3  C   ALA A   1       2.009   1.420   0.000  1.00  0.00           C\n\
END\n";

fn aniso_source() -> MolecularSource {
    let result = molframe::read_bytes(
        ANISO_PDB.as_bytes().to_vec(),
        Some("aniso.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("anisotropy fixture must parse")
    };
    MolecularSource::from_molframe(&structure)
}

#[test]
fn anisotropic_tensors_reach_the_atom_that_carries_them() {
    let source = aniso_source();
    let anisotropy = source.topology().anisotropy.as_ref();
    // Only the CA of the one residue carries a tensor, and it reaches the atom
    // row in U order scaled by the deposition factor of one ten-thousandth.
    assert_eq!(anisotropy, [(1, [10.0, 20.0, 30.0, 4.0, 5.0, 6.0])]);
}

#[test]
fn an_isotropic_structure_carries_no_anisotropy_rows() {
    assert!(source().topology().anisotropy.is_empty());
}

fn has_bond(source: &MolecularSource, a: u32, b: u32) -> bool {
    source
        .topology()
        .bonds
        .iter()
        .any(|bond| bond.atoms == [a, b] || bond.atoms == [b, a])
}

#[test]
fn file_secondary_structure_reaches_every_residue_row() {
    assert_eq!(
        source().topology().secondary_structure.as_ref(),
        [molframe::SecondaryStructure::Helix; 3]
    );
}

#[test]
fn peptide_links_join_the_carbonyl_carbon_to_the_next_nitrogen() {
    let source = source();
    assert!(has_bond(&source, 2, 4), "residue one C to residue two N");
    assert!(has_bond(&source, 6, 8), "residue two C to residue three N");
}

#[test]
fn a_file_bond_is_kept_once_even_when_distance_inference_finds_it_too() {
    let source = source();
    let count = source
        .topology()
        .bonds
        .iter()
        .filter(|bond| bond.atoms == [0, 1] || bond.atoms == [1, 0])
        .count();
    assert_eq!(count, 1);
}

#[test]
fn every_bond_joins_two_distinct_atoms_inside_the_table() {
    let source = source();
    let atoms = u32::try_from(source.topology().atoms.len()).unwrap_or(u32::MAX);
    for bond in source.topology().bonds.iter() {
        assert_ne!(bond.atoms[0], bond.atoms[1]);
        assert!(bond.atoms.iter().all(|&atom| atom < atoms));
    }
}

/// A deposited protein, nucleic-acid and ligand input, each with a known
/// connectivity that the file itself states.
///
/// The point of the fixture is a count that can be checked without a renderer:
/// how many bonds arrive from the file and how many the reading adds, and how
/// many secondary-structure rows survive. Those three numbers are what the
/// cross-engine comparison is supposed to agree on before any pixel is
/// compared.
const PROTEIN_PDB: &str = "\
ATOM      1  N   GLY A   1       0.000   0.000   0.000  1.00  0.00           N\n\
ATOM      2  CA  GLY A   1       1.458   0.000   0.000  1.00  0.00           C\n\
ATOM      3  C   GLY A   1       2.009   1.420   0.000  1.00  0.00           C\n\
ATOM      4  O   GLY A   1       1.251   2.390   0.000  1.00  0.00           O\n\
ATOM      5  N   GLY A   2       3.332   1.540   0.000  1.00  0.00           N\n\
ATOM      6  CA  GLY A   2       3.966   2.850   0.000  1.00  0.00           C\n\
ATOM      7  C   GLY A   2       5.480   2.750   0.000  1.00  0.00           C\n\
ATOM      8  O   GLY A   2       6.020   1.650   0.000  1.00  0.00           O\n\
END\n";

const NUCLEIC_PDB: &str = "\
ATOM      1  P     A A   1       0.000   0.000   0.000  1.00  0.00           P\n\
ATOM      2  O3'   A A   1       1.600   0.000   0.000  1.00  0.00           O\n\
ATOM      3  C4'   A A   1       0.700   1.400   0.000  1.00  0.00           C\n\
ATOM      4  P     A A   2       2.900   0.300   0.000  1.00  0.00           P\n\
ATOM      5  O3'   A A   2       4.500   0.300   0.000  1.00  0.00           O\n\
ATOM      6  C4'   A A   2       3.600   1.700   0.000  1.00  0.00           C\n\
END\n";

const LIGAND_PDB: &str = "\
HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00  0.00           C\n\
HETATM    2  C2  LIG A   1       1.390   0.000   0.000  1.00  0.00           C\n\
HETATM    3  O1  LIG A   1       2.020   1.090   0.000  1.00  0.00           O\n\
CONECT    1    2    3\n\
CONECT    2    1\n\
CONECT    3    1\n\
END\n";

fn read(pdb: &str, name: &str) -> MolecularSource {
    let Ok((structure, _)) = molframe::read_bytes(
        pdb.as_bytes().to_vec(),
        Some(name),
        &molframe::ReadOptions::new(),
    ) else {
        panic!("{name} must parse")
    };
    MolecularSource::from_molframe(&structure)
}

#[test]
fn file_and_inferred_bond_counts_are_deterministic_across_the_shared_fixtures() {
    // The protein file states no bonds, so every edge is inferred; the reading
    // must still produce the same count twice.
    let protein = read(PROTEIN_PDB, "protein.pdb");
    let protein_again = read(PROTEIN_PDB, "protein.pdb");
    assert_eq!(
        protein.topology().bonds.len(),
        protein_again.topology().bonds.len()
    );
    assert!(
        !protein.topology().bonds.is_empty(),
        "a three-residue peptide has at least its backbone bonds"
    );
    assert_eq!(
        protein.topology().secondary_structure.len(),
        2,
        "one secondary-structure row per residue"
    );

    // The nucleic fixture keeps its two backbone links.
    let nucleic = read(NUCLEIC_PDB, "nucleic.pdb");
    assert_eq!(nucleic.topology().atoms.len(), 6);
    assert!(!nucleic.topology().bonds.is_empty());

    // The ligand file states all three edges; each must appear exactly once
    // even though the reading can also infer them by distance.
    let ligand = read(LIGAND_PDB, "ligand.pdb");
    let stated = ligand
        .topology()
        .bonds
        .iter()
        .filter(|bond| bond.atoms == [0, 1] || bond.atoms == [1, 0])
        .count();
    assert_eq!(stated, 1, "one file edge, not a duplicate inference");
}
