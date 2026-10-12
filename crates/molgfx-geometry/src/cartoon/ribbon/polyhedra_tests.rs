use super::*;
use molgfx_core::{TrajectoryFrame, TrajectorySegment};
use std::sync::Arc;

fn placed() -> PlacedStructure {
    let pdb = b"HETATM    1  C1  UNK A   1       0.000   0.000   0.000  1.00 10.00           C\nHETATM    2  C2  UNK A   1       1.000   0.000   0.000  1.00 10.00           C\nHETATM    3  C3  UNK A   1       0.000   1.000   0.000  1.00 10.00           C\nHETATM    4  C4  UNK A   1       0.250   0.250   0.000  1.00 10.00           C\nEND\n";
    let (structure, _) = molframe::read_bytes(
        pdb.to_vec(),
        Some("triangles.pdb"),
        &molframe::ReadOptions::new(),
    )
    .unwrap();
    PlacedStructure::new(&structure).unwrap()
}

#[test]
fn polyhedra_connects_neighbor_triplets_around_the_selected_center_and_picks_that_center() {
    let placed = placed();
    let mut mesh = RibbonMesh::default();
    mesh.generate_polyhedra(
        &placed,
        &AtomSelection::All,
        1.5,
        Rgba8::opaque(255, 255, 255),
    )
    .unwrap();
    assert_eq!(mesh.indices.len(), 12);
    assert_eq!(mesh.deformations.len(), 12);
    for recipe in &mesh.deformations {
        assert_eq!(recipe.parameter[3].to_bits(), 7);
    }
    mesh.generate_polyhedra(
        &placed,
        &AtomSelection::Sparse(vec![0]),
        1.1,
        Rgba8::opaque(255, 255, 255),
    )
    .unwrap();
    assert_eq!(
        mesh.indices.len(),
        3,
        "neighbors need not be selected or close to one another"
    );
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| vertex.entity_id == EntityId::pack(EntityKind::Atom, 0).unwrap().0)
    );
    assert!(
        mesh.deformations
            .iter()
            .all(|recipe| recipe.controls == [1, 2, 3, 0])
    );
}

#[test]
fn the_candidate_stream_contains_triangles_that_appear_between_resident_frames() {
    let mut placed = placed();
    let start = Arc::from([
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 0.0],
        [0.0, 4.0, 0.0],
        [0.0, 0.0, 4.0],
    ]);
    let end = Arc::from([
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ]);
    let segment = TrajectorySegment::new(
        TrajectoryFrame::new(0, 0.0, start, "start").unwrap(),
        TrajectoryFrame::new(1, 1.0, end, "end").unwrap(),
        0.0,
    )
    .unwrap();
    // Use the public scene seam, which constructs the swept spatial hierarchy.
    let mut scene = molgfx_core::Scene::new();
    let owner = scene.add_asset(placed.asset());
    scene.set_trajectory_segment(owner, segment).unwrap();
    placed = scene.structure(owner).unwrap().clone();
    let mut mesh = RibbonMesh::default();
    mesh.generate_polyhedra(
        &placed,
        &AtomSelection::Sparse(vec![0]),
        1.5,
        Rgba8::opaque(255, 255, 255),
    )
    .unwrap();
    assert_eq!(mesh.indices.len(), 3);
}

#[test]
fn faces_never_mix_mutually_exclusive_neighbor_conformations() {
    let pdb = b"HETATM    1  C1  UNK A   1       0.000   0.000   0.000  1.00 10.00           C\nHETATM    2  O1 AUNK A   2       1.000   0.000   0.000  0.50 10.00           O\nHETATM    3  O2 BUNK A   2       0.000   1.000   0.000  0.50 10.00           O\nHETATM    4  O3  UNK A   2       0.250   0.250   0.000  1.00 10.00           O\nEND\n";
    let (source, _) = molframe::read_bytes(
        pdb.to_vec(),
        Some("alternate.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source");
    let placed = PlacedStructure::new(&source).expect("placed");
    let labels = &placed.source.topology().atoms;
    assert!(!labels[1].alt_id.is_blank() && !labels[2].alt_id.is_blank());
    assert_ne!(labels[1].alt_id, labels[2].alt_id);
    let mut mesh = RibbonMesh::default();
    mesh.generate_polyhedra(
        &placed,
        &AtomSelection::Sparse(vec![0]),
        1.5,
        Rgba8::opaque(255, 255, 255),
    )
    .expect("geometry");
    assert!(mesh.indices.is_empty());
}
