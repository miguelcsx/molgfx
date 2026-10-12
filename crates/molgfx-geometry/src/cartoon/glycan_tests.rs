use super::*;
use std::fmt::Write as _;

fn glycan(centres: &[(f32, f32, f32)], links: &[(usize, usize)]) -> molframe::Structure {
    let mut cif = String::from(
        "data_glycan\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_alt_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n_atom_site.occupancy\n_atom_site.B_iso_or_equiv\n_atom_site.auth_seq_id\n_atom_site.auth_asym_id\n_atom_site.pdbx_PDB_model_num\n",
    );
    let names = ["C1", "C2", "C3", "C4", "C5", "O5"];
    let mut id = 1;
    for (residue, &(x, y, z)) in centres.iter().enumerate() {
        let seq = residue + 1;
        for (corner, name) in names.iter().enumerate() {
            let angle =
                f32::from(u8::try_from(corner).expect("ring corner")) * std::f32::consts::TAU / 6.0;
            let element = if name.starts_with('O') { "O" } else { "C" };
            writeln!(
                cif,
                "HETATM {id} {element} {name} . NAG A 1 {seq} {} {} {z} 1.00 10.0 {seq} A 1",
                x + angle.cos() * 1.4,
                y + angle.sin() * 1.4
            )
            .expect("ring row");
            id += 1;
        }
        writeln!(
            cif,
            "HETATM {id} O O4 . NAG A 1 {seq} {} {y} {z} 1.00 10.0 {seq} A 1",
            x - 2.8
        )
        .expect("acceptor row");
        id += 1;
    }
    cif.push_str("#\nloop_\n_struct_conn.id\n_struct_conn.conn_type_id\n_struct_conn.ptnr1_label_asym_id\n_struct_conn.ptnr1_label_seq_id\n_struct_conn.ptnr1_label_comp_id\n_struct_conn.ptnr1_label_atom_id\n_struct_conn.ptnr2_label_asym_id\n_struct_conn.ptnr2_label_seq_id\n_struct_conn.ptnr2_label_comp_id\n_struct_conn.ptnr2_label_atom_id\n");
    let mut edge = 0;
    for residue in 1..=centres.len() {
        for corner in 0..6 {
            writeln!(
                cif,
                "b{edge} covale A {residue} NAG {} A {residue} NAG {}",
                names[corner],
                names[(corner + 1) % 6]
            )
            .expect("ring edge");
            edge += 1;
        }
        writeln!(cif, "b{edge} covale A {residue} NAG C4 A {residue} NAG O4")
            .expect("acceptor edge");
        edge += 1;
    }
    for &(donor, acceptor) in links {
        writeln!(
            cif,
            "b{edge} covale A {} NAG C1 A {} NAG O4",
            donor + 1,
            acceptor + 1
        )
        .expect("glycosidic edge");
        edge += 1;
    }
    molframe::read_bytes(
        cif.into_bytes(),
        Some("glycan.cif"),
        &molframe::ReadOptions::new(),
    )
    .expect("source")
    .0
}

#[test]
fn a_chemically_linked_chain_preserves_complete_ring_planes_and_source_rows() {
    let structure = glycan(
        &[(0.0, 0.0, 0.0), (5.0, 0.0, 0.0), (10.0, 0.0, 0.0)],
        &[(0, 1), (1, 2)],
    );
    let mut traces = PolymerTraces::default();
    extract_glycosidic_traces(&structure, &AtomSelection::All, &mut traces).expect("inventory");
    assert_eq!(traces.ranges().len(), 1);
    assert_eq!(traces.ranges()[0].points.len(), 3);
    assert!(
        traces
            .normals
            .iter()
            .all(|normal| normal.dot(Vec3::Z).abs() > 0.99)
    );
    assert_eq!(traces.entities.len(), 3);
}

#[test]
fn close_unlinked_rings_do_not_become_a_glycosidic_chain() {
    let structure = glycan(&[(0.0, 0.0, 0.0), (5.0, 0.0, 0.0)], &[]);
    let mut traces = PolymerTraces::default();
    extract_glycosidic_traces(&structure, &AtomSelection::All, &mut traces).expect("inventory");
    assert!(traces.ranges().is_empty());
}

#[test]
fn an_incomplete_ring_selection_cannot_supply_a_fabricated_plane() {
    let structure = glycan(&[(0.0, 0.0, 0.0), (5.0, 0.0, 0.0)], &[(0, 1)]);
    let mut traces = PolymerTraces::default();
    extract_glycosidic_traces(
        &structure,
        &AtomSelection::Range(1..structure.atom_count()),
        &mut traces,
    )
    .expect("inventory");
    assert!(traces.ranges().is_empty());
}

fn sugars(count: u8) -> Vec<Option<Sugar>> {
    (0..count)
        .map(|index| {
            Some(Sugar {
                centre: Vec3::new(f32::from(index), 0.0, 0.0),
                normal: if index % 2 == 0 { Vec3::Z } else { -Vec3::Z },
                entity: u32::from(index),
            })
        })
        .collect()
}

#[test]
fn a_branch_keeps_every_arm_once_even_when_its_centre_was_already_visited() {
    let mut traces = PolymerTraces::default();
    build_runs(
        &sugars(4),
        &[
            vec![(1, 0), (2, 1), (3, 2)],
            vec![(0, 0)],
            vec![(0, 1)],
            vec![(0, 2)],
        ],
        3,
        &mut traces,
    );
    assert_eq!(traces.ranges().len(), 3);
    assert_eq!(traces.entities, [0, 1, 0, 2, 0, 3]);
    assert_eq!(
        traces.normals,
        [Vec3::Z, -Vec3::Z, Vec3::Z, Vec3::Z, Vec3::Z, -Vec3::Z]
    );
}

#[test]
fn a_closed_cycle_keeps_its_closing_edge_once() {
    let mut traces = PolymerTraces::default();
    build_runs(
        &sugars(3),
        &[
            vec![(1, 0), (2, 2)],
            vec![(0, 0), (2, 1)],
            vec![(1, 1), (0, 2)],
        ],
        3,
        &mut traces,
    );
    assert_eq!(traces.ranges().len(), 1);
    assert_eq!(traces.entities, [0, 1, 2, 0]);
}
