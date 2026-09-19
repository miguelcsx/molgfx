use super::*;

/// A minimal cytosine-like nucleotide: a planar six-membered ring in the z = 0
/// plane plus the sugar carbon it hangs from.
const NUCLEOTIDE_CIF: &str = "\
data_base
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
ATOM 1 C \"C1'\" . DC A 1 1 -2.400 0.000 0.000 1.00 10.0 1 A 1
ATOM 2 N N1  . DC A 1 1 -1.200 0.000 0.000 1.00 10.0 1 A 1
ATOM 3 C C2  . DC A 1 1 -0.600 1.039 0.000 1.00 10.0 1 A 1
ATOM 4 N N3  . DC A 1 1  0.600 1.039 0.000 1.00 10.0 1 A 1
ATOM 5 C C4  . DC A 1 1  1.200 0.000 0.000 1.00 10.0 1 A 1
ATOM 6 C C5  . DC A 1 1  0.600 -1.039 0.000 1.00 10.0 1 A 1
ATOM 7 C C6  . DC A 1 1 -0.600 -1.039 0.000 1.00 10.0 1 A 1
";

fn nucleotide() -> molframe::Structure {
    let options = molframe::ReadOptions::new();
    match molframe::read_bytes(
        NUCLEOTIDE_CIF.as_bytes().to_vec(),
        Some("base.cif"),
        &options,
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("fixture must parse: {diagnostics:?}"),
    }
}

#[test]
fn a_planar_base_emits_a_slab_lying_in_the_ring_plane() {
    let structure = nucleotide();
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    append_base_slabs(
        &structure,
        &AtomSelection::All,
        0.5,
        &mut vertices,
        &mut indices,
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert!(!vertices.is_empty(), "a nucleotide emits geometry");
    assert_eq!(indices.len() % 3, 0, "geometry is triangulated");
    assert!(
        indices
            .iter()
            .all(|index| (*index as usize) < vertices.len()),
        "every index addresses a real vertex"
    );

    // The ring lies in z = 0, so the slab may not extend past half its own
    // thickness in z. Anything more means the plane was resolved incorrectly.
    let deepest = vertices
        .iter()
        .map(|vertex| vertex.position[2].abs())
        .fold(0.0f32, f32::max);
    assert!(deepest <= 0.26, "slab hugs the ring plane, got {deepest}");

    // The slab has to cover the ring footprint it was built from.
    let widest = vertices
        .iter()
        .map(|vertex| vertex.position[0].abs())
        .fold(0.0f32, f32::max);
    assert!(widest >= 1.2, "slab spans the ring, got {widest}");
}

/// A glycine: no ring atoms and no sugar, so nothing may be emitted.
const PROTEIN_CIF: &str = "\
data_protein
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
ATOM 1 N N  . GLY A 1 1 -0.525 1.362 0.000 1.00 10.0 1 A 1
ATOM 2 C CA . GLY A 1 1  0.000 0.000 0.000 1.00 10.0 1 A 1
ATOM 3 C C  . GLY A 1 1  1.520 0.000 0.000 1.00 10.0 1 A 1
ATOM 4 O O  . GLY A 1 1  2.197 0.995 0.000 1.00 10.0 1 A 1
";

#[test]
fn residues_without_a_base_ring_emit_nothing() {
    let options = molframe::ReadOptions::new();
    let structure = match molframe::read_bytes(
        PROTEIN_CIF.as_bytes().to_vec(),
        Some("protein.cif"),
        &options,
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("fixture must parse: {diagnostics:?}"),
    };
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    append_base_slabs(
        &structure,
        &AtomSelection::All,
        0.5,
        &mut vertices,
        &mut indices,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        vertices.is_empty() && indices.is_empty(),
        "a protein fixture carries no nucleotide slabs"
    );
}
