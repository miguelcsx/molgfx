use super::*;

#[test]
fn a_sparse_surface_selection_uses_only_its_atom_bounds() {
    let source = "data_bounds\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\nATOM 1 C CA ALA A 1 0 0 0\nATOM 2 C CA GLY A 2 100 0 0\n";
    let parsed = pdbiox::read_bytes(
        source.as_bytes().to_vec(),
        Some("bounds.cif"),
        &pdbiox::ReadOptions::new(),
    );
    let (structure, _) = match parsed {
        Ok(value) => value,
        Err(diagnostics) => panic!("bounds fixture parses: {diagnostics:?}"),
    };
    let Some(placed) = PlacedStructure::new(&structure) else {
        panic!("bounds fixture has coordinates")
    };
    let selected = selected_atom_bounds(&placed, &AtomSelection::Sparse(vec![0]));
    assert!(selected.max.x < 10.0);
    assert!(placed.render_bvh().bounds().max.x > 90.0);
}
