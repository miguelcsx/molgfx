use super::tests::{camera, engine};
use pdviewx_core::{AtomSelection, Representation, Scene, TubeRadiusMapping};

fn polymer() -> pdbiox::Structure {
    let cif = "\
data_putty
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_alt_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
_atom_site.auth_seq_id
_atom_site.auth_asym_id
_atom_site.pdbx_PDB_model_num
ATOM 1 C CA . GLY A 1 1 0.0 0.0 0.0 1.0 10.0 1 A 1
ATOM 2 C CA . ALA A 1 2 2.0 0.4 0.0 1.0 30.0 2 A 1
ATOM 3 C CA . SER A 1 3 4.0 0.0 0.0 1.0 50.0 3 A 1
";
    match pdbiox::read_bytes(
        cif.as_bytes().to_vec(),
        Some("putty-render-test.cif"),
        &pdbiox::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("putty fixture parses: {diagnostics:?}"),
    }
}

fn write_count(device: &crate::testing::MockDevice, buffer: u32) -> usize {
    device.log.writes.lock().map_or_else(
        |error| panic!("write log locks: {error}"),
        |writes| writes.iter().filter(|write| write.0 == buffer).count(),
    )
}

#[test]
fn putty_uses_one_persistent_guide_buffer_and_indirect_render_path() {
    let source = polymer();
    let mut scene = Scene::from_structure(&source)
        .unwrap_or_else(|error| panic!("putty scene builds: {error}"));
    let selection = scene.add_selection(AtomSelection::All);
    let recipe = Representation::putty([10.0, 50.0], [0.2, 0.8])
        .unwrap_or_else(|error| panic!("putty recipe validates: {error}"));
    let representation = scene
        .represent(selection, recipe)
        .unwrap_or_else(|error| panic!("putty representation applies: {error}"));
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("putty frame renders: {error}"));

    let radius_buffer = engine
        .device
        .log
        .buffers
        .lock()
        .map_or_else(
            |error| panic!("buffer log locks: {error}"),
            |buffers| {
                buffers
                    .iter()
                    .find(|(_, label, _)| *label == "cartoon guide radius sources")
                    .map(|(id, _, size)| (*id, *size))
            },
        )
        .unwrap_or_else(|| panic!("putty source buffer exists"));
    assert_eq!(
        radius_buffer.1, 256,
        "small source streams use retained capacity"
    );
    assert_eq!(write_count(&engine.device, radius_buffer.0), 1);
    let buffers_after_warmup = engine.device.log.buffers.lock().map_or_else(
        |error| panic!("buffer log locks: {error}"),
        |buffers| buffers.len(),
    );

    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("stable putty frame renders: {error}"));
    assert_eq!(write_count(&engine.device, radius_buffer.0), 1);
    assert_eq!(
        engine.device.log.buffers.lock().map_or_else(
            |error| panic!("buffer log locks: {error}"),
            |buffers| buffers.len()
        ),
        buffers_after_warmup,
        "stable frames allocate no GPU resources"
    );

    let Some(value) = scene.representation_mut(representation) else {
        panic!("putty representation resolves")
    };
    value.params.tube_radius_mapping = TubeRadiusMapping::b_factor([0.0, 60.0], [0.1, 1.0])
        .unwrap_or_else(|error| panic!("replacement mapping validates: {error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("remapped putty frame renders: {error}"));
    assert_eq!(
        write_count(&engine.device, radius_buffer.0),
        1,
        "mapping changes update uniforms without rebuilding the source stream"
    );
    assert_eq!(
        engine.device.log.indirect_draws.lock().map_or_else(
            |error| panic!("draw log locks: {error}"),
            |draws| draws.len()
        ),
        6,
        "shadow and beauty stay indirect across all three frames"
    );
}
