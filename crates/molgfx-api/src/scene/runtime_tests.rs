use super::structure_hash;

fn parse(source: &str) -> molframe::Structure {
    let result = molframe::read_bytes(
        source.as_bytes().to_vec(),
        Some("identity.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("fixture must parse")
    };
    structure
}

#[test]
fn structure_identity_includes_topology_not_only_coordinates() {
    let alanine = parse(
        "ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N\nEND\n",
    );
    let glycine = parse(
        "ATOM      1  N   GLY A   1      11.104   6.134  -6.504  1.00  0.00           N\nEND\n",
    );
    assert_ne!(
        structure_hash(&molgfx_core::MolecularSource::from_molframe(&alanine)),
        structure_hash(&molgfx_core::MolecularSource::from_molframe(&glycine))
    );
}
