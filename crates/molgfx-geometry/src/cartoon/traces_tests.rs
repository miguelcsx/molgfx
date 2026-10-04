use super::*;
use std::fmt::Write;

fn structure(sequence: [i32; 4], author: [i32; 4]) -> molframe::Structure {
    let mut cif = String::from(
        "data_trace\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_alt_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n_atom_site.occupancy\n_atom_site.B_iso_or_equiv\n_atom_site.auth_seq_id\n_atom_site.auth_asym_id\n_atom_site.pdbx_PDB_model_num\n",
    );
    for (index, (sequence, author)) in sequence.into_iter().zip(author).enumerate() {
        assert!(
            writeln!(
                cif,
                "ATOM {} C CA . GLY A 1 {sequence} {} 0 0 1 10 {author} A 1",
                index + 1,
                index * 2
            )
            .is_ok()
        );
    }
    match molframe::read_bytes(
        cif.into_bytes(),
        Some("trace.cif"),
        &molframe::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(error) => panic!("trace fixture parses: {error:?}"),
    }
}

#[test]
fn missing_canonical_residues_split_spatially_close_guides() {
    let structure = structure([1, 2, 6, 7], [1, 2, 6, 7]);
    let mut traces = PolymerTraces::default();
    assert!(
        extract_polymer_traces(
            &structure,
            &molgfx_core::AtomSelection::All,
            &[],
            CARTOON_GAP_CUTOFF,
            &mut traces
        )
        .is_ok()
    );
    assert_eq!(
        traces
            .ranges()
            .iter()
            .map(|range| range.points.clone())
            .collect::<Vec<_>>(),
        vec![0..2, 2..4]
    );
}

#[test]
fn author_numbering_gaps_do_not_split_canonical_neighbors() {
    let structure = structure([1, 2, 3, 4], [10, 20, 30, 40]);
    let mut traces = PolymerTraces::default();
    assert!(
        extract_polymer_traces(
            &structure,
            &molgfx_core::AtomSelection::All,
            &[],
            CARTOON_GAP_CUTOFF,
            &mut traces
        )
        .is_ok()
    );
    assert_eq!(traces.ranges()[0].points, 0..4);
    assert_eq!(traces.ranges().len(), 1);
}
