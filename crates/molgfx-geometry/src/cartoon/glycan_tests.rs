use super::*;
use std::fmt::Write as _;

/// Builds an mmCIF of pyranose rings whose centres sit at the given points.
/// Each ring is a small hexagon around its centre, which is all the extractor
/// reads.
fn glycan(centres: &[(f32, f32, f32)]) -> molframe::Structure {
    let mut cif = String::from(
        "data_glycan\nloop_\n_atom_site.group_PDB\n_atom_site.id\n\
_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_alt_id\n\
_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_entity_id\n\
_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n\
_atom_site.occupancy\n_atom_site.B_iso_or_equiv\n_atom_site.auth_seq_id\n\
_atom_site.auth_asym_id\n_atom_site.pdbx_PDB_model_num\n",
    );
    let names = ["C1", "C2", "C3", "C4", "C5", "O5"];
    let mut id = 1;
    for (residue, (x, y, z)) in centres.iter().enumerate() {
        let seq = residue + 1;
        for (corner, name) in names.iter().enumerate() {
            let mut angle = 0.0f32;
            let mut step = 0;
            while step < corner {
                angle += std::f32::consts::TAU / 6.0;
                step += 1;
            }
            let element = if name.starts_with('O') { "O" } else { "C" };
            let _ = writeln!(
                cif,
                "HETATM {id} {element} {name} . NAG A 1 {seq} {} {} {} 1.00 10.0 {seq} A 1",
                x + angle.cos() * 1.4,
                y + angle.sin() * 1.4,
                z,
            );
            id += 1;
        }
    }
    let options = molframe::ReadOptions::new();
    match molframe::read_bytes(cif.into_bytes(), Some("glycan.cif"), &options) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("glycan fixture parses: {diagnostics:?}"),
    }
}

#[test]
fn a_linear_chain_of_sugars_becomes_one_trace() {
    let structure = glycan(&[(0.0, 0.0, 0.0), (5.0, 0.0, 0.0), (10.0, 0.0, 0.0)]);
    let mut traces = PolymerTraces::default();
    extract_glycosidic_traces(&structure, &AtomSelection::All, &mut traces);
    assert_eq!(traces.ranges().len(), 1, "an unbranched glycan is one run");
    let Some(range) = traces.ranges().first() else {
        panic!("one range")
    };
    assert_eq!(range.points.len(), 3, "every sugar contributes a point");
}

#[test]
fn a_branch_point_splits_the_path_into_separate_runs() {
    // A centre sugar carrying two others: one run per arm.
    let structure = glycan(&[
        (0.0, 0.0, 0.0),
        (5.0, 0.0, 0.0),
        (5.0, 5.0, 0.0),
        (10.0, 0.0, 0.0),
    ]);
    let mut traces = PolymerTraces::default();
    extract_glycosidic_traces(&structure, &AtomSelection::All, &mut traces);
    assert!(
        traces.ranges().len() >= 2,
        "a branched glycan yields more than one run, got {}",
        traces.ranges().len()
    );
}

#[test]
fn sugars_too_far_apart_are_not_linked() {
    let structure = glycan(&[(0.0, 0.0, 0.0), (40.0, 0.0, 0.0)]);
    let mut traces = PolymerTraces::default();
    extract_glycosidic_traces(&structure, &AtomSelection::All, &mut traces);
    assert!(
        traces.ranges().is_empty(),
        "unlinked sugars draw no ribbon between them"
    );
}

#[test]
fn a_structure_without_sugar_rings_yields_nothing() {
    let options = molframe::ReadOptions::new();
    let cif = "data_p\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n\
_atom_site.label_atom_id\n_atom_site.label_alt_id\n_atom_site.label_comp_id\n\
_atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n\
_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n_atom_site.occupancy\n\
_atom_site.B_iso_or_equiv\n_atom_site.auth_seq_id\n_atom_site.auth_asym_id\n\
_atom_site.pdbx_PDB_model_num\n\
ATOM 1 N N . GLY A 1 1 0.0 0.0 0.0 1.00 10.0 1 A 1\n\
ATOM 2 C CA . GLY A 1 1 1.5 0.0 0.0 1.00 10.0 1 A 1\n";
    let Ok((structure, _)) = molframe::read_bytes(cif.as_bytes().to_vec(), Some("p.cif"), &options)
    else {
        panic!("protein fixture parses")
    };
    let mut traces = PolymerTraces::default();
    extract_glycosidic_traces(&structure, &AtomSelection::All, &mut traces);
    assert!(traces.ranges().is_empty());
}

/// A planar hexagon in the xy-plane, wound counter-clockwise, at `z`.
fn hexagon(z: f32) -> Vec<Vec3> {
    (0..6u8)
        .map(|corner| {
            let angle = std::f32::consts::TAU / 6.0 * f32::from(corner);
            Vec3::new(angle.cos() * 1.4, angle.sin() * 1.4, z)
        })
        .collect()
}

#[test]
fn a_flat_ring_reports_the_plane_it_lies_in() {
    let Some(normal) = ring_normal(&hexagon(0.0)) else {
        panic!("a flat hexagon has a plane")
    };
    assert!(
        (normal.abs() - Vec3::Z).length() < 1.0e-5,
        "a ring in the xy-plane is normal to z, got {normal:?}"
    );
    assert!(
        (normal.length() - 1.0).abs() < 1.0e-5,
        "the plane is a unit normal"
    );
}

#[test]
fn the_reported_plane_is_the_same_wherever_the_ring_sits() {
    // Newell's sum is translation-invariant, which is what lets a sugar far
    // from the origin be oriented against one near it.
    let (Some(near), Some(far)) = (ring_normal(&hexagon(0.0)), ring_normal(&hexagon(120.0))) else {
        panic!("both rings have a plane")
    };
    assert!(
        (near - far).length() < 1.0e-5,
        "{near:?} and {far:?} differ"
    );
}

#[test]
fn reversing_the_winding_reverses_the_reported_plane() {
    // This is the ambiguity the walker has to resolve: the same ring reports
    // either face depending only on the order its atoms were listed in.
    let mut ring = hexagon(0.0);
    let Some(forward) = ring_normal(&ring) else {
        panic!("a hexagon has a plane")
    };
    ring.reverse();
    let Some(backward) = ring_normal(&ring) else {
        panic!("a reversed hexagon has a plane")
    };
    assert!(
        (forward + backward).length() < 1.0e-5,
        "{forward:?} and {backward:?} are not opposite"
    );
}

#[test]
fn a_collapsed_ring_reports_no_plane_rather_than_rounding_error() {
    assert!(ring_normal(&[Vec3::ZERO; 6]).is_none());
}

#[test]
fn every_sugar_on_a_trace_carries_its_chemically_ordered_plane() {
    let structure = glycan(&[(0.0, 0.0, 0.0), (5.0, 0.0, 0.0), (10.0, 0.0, 0.0)]);
    let mut traces = PolymerTraces::default();
    extract_glycosidic_traces(&structure, &AtomSelection::All, &mut traces);
    let Some(range) = traces.ranges().first().cloned() else {
        panic!("one range")
    };
    let normals = &traces.normals[range.points.clone()];
    assert_eq!(normals.len(), range.points.len(), "one plane per sugar");
    assert!(normals.iter().all(|normal| normal.length() > 0.99));
}

#[test]
fn appending_a_sugar_preserves_an_opposite_chemical_orientation() {
    let sugars = [Sugar {
        chain: 0,
        centre: Vec3::ZERO,
        normal: -Vec3::Z,
        entity: 1,
        atoms: Vec::new(),
    }];
    let mut points = Vec::new();
    let mut entities = Vec::new();
    let mut normals = vec![Vec3::Z];
    let mut visited = [false];
    push_sugar(
        &sugars,
        0,
        &mut points,
        &mut entities,
        &mut normals,
        &mut visited,
    );
    assert_eq!(normals.last(), Some(&-Vec3::Z));
}
