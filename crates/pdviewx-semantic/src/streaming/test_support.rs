use pdviewx_core::{Scene, StructureHandle};

pub(super) fn structure() -> pdbiox::Structure {
    let cif = "data_lod\n\
loop_\n\
_atom_site.group_PDB\n\
_atom_site.id\n\
_atom_site.type_symbol\n\
_atom_site.label_atom_id\n\
_atom_site.label_alt_id\n\
_atom_site.label_comp_id\n\
_atom_site.label_asym_id\n\
_atom_site.label_entity_id\n\
_atom_site.label_seq_id\n\
_atom_site.Cartn_x\n\
_atom_site.Cartn_y\n\
_atom_site.Cartn_z\n\
_atom_site.occupancy\n\
_atom_site.B_iso_or_equiv\n\
_atom_site.auth_seq_id\n\
_atom_site.auth_asym_id\n\
_atom_site.pdbx_PDB_model_num\n\
ATOM 1 C CA . GLY A 1 1 0 0 0 1 10 1 A 1\n";
    match pdbiox::read_bytes(
        cif.as_bytes().to_vec(),
        Some("lod.cif"),
        &pdbiox::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("LOD fixture parses: {diagnostics:?}"),
    }
}

pub(super) fn handle() -> StructureHandle {
    let mut scene = Scene::new();
    match scene.add_structure(&structure()) {
        Ok(handle) => handle,
        Err(error) => panic!("LOD fixture enters a scene: {error}"),
    }
}
