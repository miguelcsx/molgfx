use super::*;
use molgfx_core::AtomSelection;

fn scene() -> Scene {
    let source = "data_zone\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\nATOM 1 C CA ALA A 1 0 0 0\nATOM 2 O O ALA A 1 1 0 0\nHETATM 3 C C1 LIG A 2 3 0 0\nATOM 4 C CA GLY A 3 12 0 0\n";
    let parsed = molframe::read_bytes(
        source.as_bytes().to_vec(),
        Some("zone.cif"),
        &molframe::ReadOptions::default(),
    );
    let (structure, _) = match parsed {
        Ok(value) => value,
        Err(diagnostics) => panic!("zone fixture parses: {diagnostics:?}"),
    };
    match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("zone scene builds: {error}"),
    }
}

#[test]
fn a_surface_zone_intersects_source_atoms_with_the_anchor_neighbourhood() {
    let mut scene = scene();
    let protein = scene.add_selection(AtomSelection::Sparse(vec![0, 1, 3]));
    let ligand = scene.add_selection(AtomSelection::Sparse(vec![2]));
    let zone = match scene.surface_zone(protein, ligand) {
        Ok(zone) => zone,
        Err(error) => panic!("surface zone composes: {error}"),
    };
    let Some(selection) = scene.selection(zone.selection) else {
        panic!("zone selection resolves")
    };
    assert_eq!(
        selection.to_bitmap(4).iter().collect::<Vec<_>>(),
        vec![0, 1]
    );
    let Some(representation) = scene.representation(zone.representation) else {
        panic!("zone representation resolves")
    };
    assert_eq!(
        representation.params.surface_kind,
        SurfaceKind::SolventExcluded
    );
    assert!((representation.material.opacity - 0.35).abs() < f32::EPSILON);
}
