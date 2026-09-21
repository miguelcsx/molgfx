// Determinism of the parallel branch: the packed instance stream must not
// depend on how the work was partitioned across threads.
//
// The index-to-float casts below build fixture coordinates from a loop
// counter bounded by a fixed literal in this file; nothing truncates.

use super::*;
use molgfx_core::{AtomSelection, RepresentationKind, Scene};
use std::fmt::Write as _;

#[test]
fn a_parallel_pack_matches_the_serial_pack() {
    // One table comfortably above the parallel threshold. The parallel branch
    // packs every selected row through `par_iter`; the serial reference packs
    // the same rows in below-threshold chunks through the serial branch, so a
    // thread-count-dependent difference in ordering, rounding or record
    // construction would show up here.
    let count = 2 * PARALLEL_BLOCK + 17;
    let mut cif = String::from(
        "data_parallel\nloop_\n_atom_site.group_PDB\n_atom_site.id\n\
         _atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_alt_id\n\
         _atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_entity_id\n\
         _atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n\
         _atom_site.Cartn_z\n_atom_site.occupancy\n_atom_site.B_iso_or_equiv\n\
         _atom_site.auth_seq_id\n_atom_site.auth_asym_id\n_atom_site.pdbx_PDB_model_num\n",
    );
    for index in 0..count {
        let _ = writeln!(
            cif,
            "ATOM {} N N . GLY A 1 {} {:.2} {:.2} {:.2} 1.00 {:.1} {} A 1",
            index + 1,
            index + 1,
            f64::from(u32::try_from(index).expect("fixture index fits u32")) * 0.13,
            f64::from(u32::try_from(index % 97).expect("fixture index fits u32")) * 0.07,
            f64::from(u32::try_from(index % 13).expect("fixture index fits u32")) * 0.31,
            10.0 + f64::from(u32::try_from(index % 29).expect("fixture index fits u32")) * 0.5,
            index + 1,
        );
    }
    let options = molframe::ReadOptions::new();
    let (structure, _) = match molframe::read_bytes(cif.into_bytes(), Some("t.cif"), &options) {
        Ok(parsed) => parsed,
        Err(diagnostics) => panic!("large fixture parses: {diagnostics:?}"),
    };
    let mut scene = match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("large scene builds: {error}"),
    };
    let selection = scene.add_selection(AtomSelection::All);
    let representation = match scene.represent(selection, RepresentationKind::Spacefill) {
        Ok(handle) => handle,
        Err(error) => panic!("spacefill applies: {error}"),
    };
    let Some(table) = scene.first_atoms() else {
        panic!("scene has atoms")
    };
    let Some(representation) = scene.representation(representation) else {
        panic!("representation resolves")
    };

    let mut parallel = Vec::new();
    pack_atoms(table, representation, &AtomSelection::All, &mut parallel)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(parallel.len(), count);

    // The same rows through the serial branch, in below-threshold chunks;
    // packing clears its scratch, so each chunk lands in its own vector.
    let mut serial = Vec::new();
    let chunk = 512usize;
    for start in (0..count).step_by(chunk) {
        let end = (start + chunk).min(count);
        let rows = AtomSelection::Range(
            u32::try_from(start).expect("fixture start fits u32")
                ..u32::try_from(end).expect("fixture end fits u32"),
        );
        let mut chunk_records = Vec::new();
        pack_atoms(table, representation, &rows, &mut chunk_records)
            .unwrap_or_else(|error| panic!("{error}"));
        serial.append(&mut chunk_records);
    }

    assert_eq!(parallel.len(), serial.len());
    for (parallel_record, serial_record) in parallel.iter().zip(&serial) {
        assert_eq!(
            bytemuck::bytes_of(parallel_record),
            bytemuck::bytes_of(serial_record),
            "parallel packing must be byte-identical to the serial pack"
        );
    }
}
