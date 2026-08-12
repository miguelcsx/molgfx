//! Test fixture: a tiny parsed structure shared across the crate's tests.

/// A minimal mmCIF: one chain with two residues (glycine, then a HETATM
/// water), eight atoms total, one model.
const FIXTURE_CIF: &str = "\
data_test
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_alt_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
_atom_site.auth_seq_id
_atom_site.auth_asym_id
_atom_site.pdbx_PDB_model_num
ATOM   1 N N   . GLY A 1 1 -0.525  1.362 0.000 1.00 10.0 1 A 1
ATOM   2 C CA  . GLY A 1 1  0.000  0.000 0.000 1.00 10.0 1 A 1
ATOM   3 C C   . GLY A 1 1  1.520  0.000 0.000 1.00 10.0 1 A 1
ATOM   4 O O   . GLY A 1 1  2.197  0.995 0.000 1.00 10.0 1 A 1
ATOM   5 H H   . GLY A 1 1 -1.525  1.362 0.100 1.00 10.0 1 A 1
ATOM   6 O OXT . GLY A 1 1  2.100 -1.180 0.000 1.00 10.0 1 A 1
HETATM 7 O O   . HOH A 2 . 5.000  5.000 5.000 1.00 20.0 2 A 1
HETATM 8 S S1  . SO4 A 3 . 8.000  1.000 2.000 1.00 30.0 3 A 1
";

/// Parses the fixture; panics with the diagnostics on failure (tests only).
pub fn structure() -> pdbiox::Structure {
    let options = pdbiox::ReadOptions::new();
    match pdbiox::read_bytes(
        FIXTURE_CIF.as_bytes().to_vec(),
        Some("fixture.cif"),
        &options,
    ) {
        Ok((structure, _diagnostics)) => structure,
        Err(diagnostics) => panic!("fixture must parse: {diagnostics:?}"),
    }
}
