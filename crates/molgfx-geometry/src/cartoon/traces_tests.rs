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

#[test]
fn guide_names_cannot_turn_calcium_ions_into_a_protein_trace() {
    let cif = "data_ions\nloop_\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n1 Ca CA CA A 1 0 0 0\n2 Ca CA CA A 2 2 0 0\n";
    let (structure, _) = molframe::read_bytes(
        cif.as_bytes().to_vec(),
        Some("ions.cif"),
        &molframe::ReadOptions::new(),
    )
    .expect("ion fixture");
    for phosphate in [false, true] {
        let mut traces = PolymerTraces::default();
        extract_backbone_traces(
            &structure,
            &molgfx_core::AtomSelection::All,
            &[],
            CARTOON_GAP_CUTOFF,
            phosphate,
            &mut traces,
        )
        .expect("valid empty traces");
        assert!(traces.ranges().is_empty());
        assert!(traces.points.is_empty());
    }
}

#[test]
fn selecting_the_second_conformer_keeps_its_guides_and_compatible_peptide_planes() {
    let cif = "data_alt\nloop_\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_alt_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n1 C CA A GLY A 1 0 0 0\n2 C CA B GLY A 1 0 1 0\n3 O O A GLY A 1 0 0 1\n4 O O B GLY A 1 0 1 1\n5 C CA A GLY A 2 3 0 0\n6 C CA B GLY A 2 3 1 0\n7 O O A GLY A 2 3 0 1\n8 O O B GLY A 2 3 1 1\n";
    let (structure, _) = molframe::read_bytes(
        cif.as_bytes().to_vec(),
        Some("alt.cif"),
        &molframe::ReadOptions::new(),
    )
    .expect("conformers");
    let selection = molgfx_core::AtomSelection::Sparse(vec![1, 5]);
    for contextual in [false, true] {
        let mut traces = PolymerTraces::default();
        if contextual {
            extract_phosphate_traces(&structure, &selection, &[], 8.0, &mut traces)
                .expect("selected context");
        } else {
            extract_polymer_traces(&structure, &selection, &[], 8.0, &mut traces)
                .expect("selected cartoon");
        }
        assert_eq!(traces.ranges.len(), 1);
        assert_eq!(
            traces.points,
            [Vec3::new(0.0, 1.0, 0.0), Vec3::new(3.0, 1.0, 0.0)]
        );
        assert_eq!(traces.plane_rows, [3, 7]);
    }
    let mut incompatible = PolymerTraces::default();
    extract_polymer_traces(
        &structure,
        &molgfx_core::AtomSelection::Sparse(vec![0, 5]),
        &[],
        8.0,
        &mut incompatible,
    )
    .expect("disjoint conformers");
    assert!(incompatible.ranges.is_empty());
    let mut single = PolymerTraces::default();
    extract_phosphate_traces(
        &structure,
        &molgfx_core::AtomSelection::Sparse(vec![5]),
        &[],
        8.0,
        &mut single,
    )
    .expect("one selected alternate guide");
    assert_eq!(single.ranges.len(), 1);
    assert_eq!(
        single.points,
        [Vec3::new(0.0, 1.0, 0.0), Vec3::new(3.0, 1.0, 0.0)]
    );
    assert_eq!(single.visible_parameters, [[0.5, 1.0]]);
}
