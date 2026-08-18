use super::*;
use std::fmt::Write as _;

/// Builds an mmCIF of pyranose rings whose centres sit at the given points.
/// Each ring is a small hexagon around its centre, which is all the extractor
/// reads.
fn glycan(centres: &[(f32, f32, f32)]) -> pdbiox::Structure {
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
    let options = pdbiox::ReadOptions::new();
    match pdbiox::read_bytes(cif.into_bytes(), Some("glycan.cif"), &options) {
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
    let options = pdbiox::ReadOptions::new();
    let cif = "data_p\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n\
_atom_site.label_atom_id\n_atom_site.label_alt_id\n_atom_site.label_comp_id\n\
_atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n\
_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n_atom_site.occupancy\n\
_atom_site.B_iso_or_equiv\n_atom_site.auth_seq_id\n_atom_site.auth_asym_id\n\
_atom_site.pdbx_PDB_model_num\n\
ATOM 1 N N . GLY A 1 1 0.0 0.0 0.0 1.00 10.0 1 A 1\n\
ATOM 2 C CA . GLY A 1 1 1.5 0.0 0.0 1.00 10.0 1 A 1\n";
    let Ok((structure, _)) = pdbiox::read_bytes(cif.as_bytes().to_vec(), Some("p.cif"), &options)
    else {
        panic!("protein fixture parses")
    };
    let mut traces = PolymerTraces::default();
    extract_glycosidic_traces(&structure, &AtomSelection::All, &mut traces);
    assert!(traces.ranges().is_empty());
}
