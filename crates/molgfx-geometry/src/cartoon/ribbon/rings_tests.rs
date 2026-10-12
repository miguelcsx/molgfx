use super::*;

fn source() -> molframe::Structure {
    let pdb = b"HETATM    1  X1  UNK A   1       1.400   0.000   0.000  1.00 10.00           C\nHETATM    2  X2  UNK A   1       0.700   1.212   0.000  1.00 10.00           C\nHETATM    3  X3  UNK A   1      -0.700   1.212   0.000  1.00 10.00           C\nHETATM    4  X4  UNK A   1      -1.400   0.000   0.000  1.00 10.00           C\nHETATM    5  X5  UNK A   1      -0.700  -1.212   0.000  1.00 10.00           C\nHETATM    6  X6  UNK A   1       0.700  -1.212   0.000  1.00 10.00           C\nCONECT    1    2    6\nCONECT    2    3\nCONECT    3    4\nCONECT    4    5\nCONECT    5    6\nEND\n";
    molframe::read_bytes(
        pdb.to_vec(),
        Some("ring.pdb"),
        &molframe::ReadOptions::new(),
    )
    .unwrap()
    .0
}

#[test]
fn an_arbitrarily_named_covalent_cycle_generates_one_bipyramid_with_exact_height() {
    let structure = source();
    let mut mesh = RibbonMesh::default();
    mesh.generate_ring_bipyramids(&structure, &AtomSelection::All, 2.0, 6, 255)
        .unwrap();
    assert_eq!(
        mesh.indices.len(),
        36,
        "two triangular faces per edge; symmetry mappings deduplicate"
    );
    assert_eq!(mesh.vertices.len(), mesh.deformations.len());
    let minimum = mesh
        .vertices
        .iter()
        .map(|vertex| vertex.position[2])
        .fold(f32::INFINITY, f32::min);
    let maximum = mesh
        .vertices
        .iter()
        .map(|vertex| vertex.position[2])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!((maximum - minimum - 2.0).abs() < 1e-6);
    assert!(
        mesh.deformations
            .iter()
            .all(|recipe| recipe.parameter[3].to_bits() == 8)
    );
    assert_eq!(mesh.radius_source_values().len(), 12 + 1 + 6);
}

#[test]
fn a_partial_selection_or_smaller_ring_limit_does_not_close_a_ring() {
    let structure = source();
    let mut mesh = RibbonMesh::default();
    mesh.generate_ring_bipyramids(
        &structure,
        &AtomSelection::Sparse(vec![0, 1, 2, 3, 4]),
        2.0,
        6,
        255,
    )
    .unwrap();
    assert!(mesh.indices.is_empty());
    mesh.generate_ring_bipyramids(&structure, &AtomSelection::All, 2.0, 5, 255)
        .unwrap();
    assert!(mesh.indices.is_empty());
}

#[test]
fn ring_geometry_rejects_invalid_controls_before_replacing_existing_vertices() {
    let structure = source();
    let mut mesh = RibbonMesh::default();
    mesh.generate_ring_bipyramids(&structure, &AtomSelection::All, 2.0, 6, 255)
        .unwrap();
    let previous = mesh.vertices.clone();
    assert!(
        mesh.generate_ring_bipyramids(&structure, &AtomSelection::All, f32::NAN, 6, 255)
            .is_err()
    );
    assert_eq!(mesh.vertices, previous);
}
