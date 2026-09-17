use super::tests::{camera, engine, structure};
use molgfx_core::{
    AtomSelection, BondTopologyFrame, BondTopologySegment, RepresentationKind, Scene, TopologyBond,
    TrajectoryFrame, TrajectorySegment,
};
use std::sync::Arc;

fn trajectory_scene() -> (Scene, molgfx_core::StructureHandle, [usize; 2]) {
    let source = structure();
    let mut scene = Scene::new();
    let handle = scene
        .add_structure(&source)
        .unwrap_or_else(|error| panic!("{error}"));
    let selection = scene.add_selection(AtomSelection::All);
    scene
        .represent(selection, RepresentationKind::Spacefill)
        .unwrap_or_else(|error| panic!("{error}"));
    let start: Arc<[[f32; 3]]> = Arc::from([[-2.0, 0.0, 0.0], [0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
    let end: Arc<[[f32; 3]]> = Arc::from([[-2.0, 2.0, 0.0], [0.0, -2.0, 0.0], [2.0, 2.0, 0.0]]);
    let pointers = [start.as_ptr() as usize, end.as_ptr() as usize];
    let start =
        TrajectoryFrame::new(8, 0.0, start, "test:start").unwrap_or_else(|error| panic!("{error}"));
    let end =
        TrajectoryFrame::new(9, 1.0, end, "test:end").unwrap_or_else(|error| panic!("{error}"));
    let segment =
        TrajectorySegment::new(start, end, 0.25).unwrap_or_else(|error| panic!("{error}"));
    scene
        .set_trajectory_segment(handle, segment)
        .unwrap_or_else(|error| panic!("{error}"));
    (scene, handle, pointers)
}

#[test]
fn resident_frames_upload_directly_and_dispatch_once_when_the_sample_changes() {
    let (mut scene, handle, pointers) = trajectory_scene();
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(pointers.into_iter().all(|pointer| {
        writes
            .iter()
            .any(|(_, _, byte_len, source)| *byte_len == 36 && *source == pointer)
    }));
    drop(writes);
    let after_first = engine
        .device
        .log
        .dispatches
        .lock()
        .map_or_else(|error| panic!("{error}"), |values| values.len());
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let after_steady = engine
        .device
        .log
        .dispatches
        .lock()
        .map_or_else(|error| panic!("{error}"), |values| values.len());
    assert_eq!(after_steady - after_first, 2, "only culling repeats");
    scene
        .set_trajectory_time(handle, 0.75)
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let after_advance = engine
        .device
        .log
        .dispatches
        .lock()
        .map_or_else(|error| panic!("{error}"), |values| values.len());
    assert_eq!(
        after_advance - after_steady,
        3,
        "interpolation plus culling"
    );
}

#[test]
fn time_only_advances_upload_one_uniform_beside_the_frame_uniforms() {
    let (mut scene, handle, _) = trajectory_scene();
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let before = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());
    scene
        .set_trajectory_time(handle, 0.5)
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        writes.len() - before,
        2,
        "new writes: {:?}",
        &writes[before..]
    );
}

#[test]
fn cartoon_time_advances_never_rebuild_or_reupload_ribbon_geometry() {
    let source = cartoon_structure();
    let mut scene = Scene::new();
    let handle = scene
        .add_structure(&source)
        .unwrap_or_else(|error| panic!("{error}"));
    let selection = scene.add_selection(AtomSelection::All);
    scene
        .represent(selection, RepresentationKind::Cartoon)
        .unwrap_or_else(|error| panic!("{error}"));
    let start = TrajectoryFrame::new(
        0,
        0.0,
        Arc::from([
            [0.0, 0.0, 0.0],
            [2.0, 0.4, 0.0],
            [4.0, 0.0, 0.0],
            [6.0, -0.4, 0.0],
        ]),
        "cartoon:start",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let end = TrajectoryFrame::new(
        1,
        1.0,
        Arc::from([
            [0.0, 0.0, 0.0],
            [2.0, 1.4, 0.2],
            [4.0, -1.0, 0.4],
            [6.0, 0.6, 0.0],
        ]),
        "cartoon:end",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    scene
        .set_trajectory_segment(
            handle,
            TrajectorySegment::new(start, end, 0.25).unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let before = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());
    scene
        .set_trajectory_time(handle, 0.75)
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        writes.len() - before,
        2,
        "only trajectory and frame uniforms change: {:?}",
        &writes[before..]
    );
}

fn cartoon_structure() -> pdbiox::Structure {
    let cif = "\
data_cartoon
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
ATOM 1 C CA . GLY A 1 1 0.0 0.0 0.0 1.00 10.0 1 A 1
ATOM 2 C CA . ALA A 1 2 2.0 0.4 0.0 1.00 10.0 2 A 1
ATOM 3 C CA . SER A 1 3 4.0 0.0 0.0 1.00 10.0 3 A 1
ATOM 4 C CA . LEU A 1 4 6.0 -0.4 0.0 1.00 10.0 4 A 1
";
    pdbiox::read_bytes(
        cif.as_bytes().to_vec(),
        Some("cartoon-trajectory.cif"),
        &pdbiox::ReadOptions::new(),
    )
    .map_or_else(
        |diagnostics| panic!("cartoon fixture parses: {diagnostics:?}"),
        |(structure, _)| structure,
    )
}

#[test]
fn replacing_shared_storage_reuploads_frames_even_when_logical_ids_are_reused() {
    let (mut scene, handle, _) = trajectory_scene();
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let before = engine.device.log.writes.lock().map_or_else(
        |error| panic!("{error}"),
        |writes| writes.iter().filter(|write| write.2 == 36).count(),
    );
    let start = TrajectoryFrame::new(
        8,
        0.0,
        Arc::from([[1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]]),
        "test:replacement-start",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let end = TrajectoryFrame::new(
        9,
        1.0,
        Arc::from([[1.0, 1.0, 0.0], [2.0, 1.0, 0.0], [3.0, 1.0, 0.0]]),
        "test:replacement-end",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    scene
        .set_trajectory_segment(
            handle,
            TrajectorySegment::new(start, end, 0.5).unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let after = engine.device.log.writes.lock().map_or_else(
        |error| panic!("{error}"),
        |writes| writes.iter().filter(|write| write.2 == 36).count(),
    );
    assert_eq!(after - before, 2);
}

#[test]
fn topology_only_ticks_never_reupload_stable_atom_records() {
    let source = structure();
    let mut scene = Scene::new();
    let handle = scene
        .add_structure(&source)
        .unwrap_or_else(|error| panic!("{error}"));
    let selection = scene.add_selection(AtomSelection::All);
    scene
        .represent(selection, RepresentationKind::BallAndStick)
        .unwrap_or_else(|error| panic!("{error}"));
    let start_bond = TopologyBond::new(0, 1, false).unwrap_or_else(|error| panic!("{error}"));
    let end_bond = TopologyBond::new(1, 2, false).unwrap_or_else(|error| panic!("{error}"));
    let start = BondTopologyFrame::new(0, 0.0, 3, Arc::from([start_bond]), "start")
        .unwrap_or_else(|error| panic!("{error}"));
    let end = BondTopologyFrame::new(1, 1.0, 3, Arc::from([end_bond]), "end")
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .set_bond_topology_segment(
            handle,
            BondTopologySegment::new(start, end, 0.25).unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let (before, atom_buffer) = engine.device.log.writes.lock().map_or_else(
        |error| panic!("{error}"),
        |writes| {
            let atom_buffer = writes
                .iter()
                .find(|write| write.2 == 3 * std::mem::size_of::<molgfx_core::AtomGpu>())
                .map_or_else(|| panic!("atom upload"), |write| write.0);
            (writes.len(), atom_buffer)
        },
    );
    scene
        .set_bond_topology_time(handle, 0.75)
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        writes.len() > before + 1,
        "topology adds work beside frame uniforms"
    );
    assert!(
        writes[before..].iter().all(|write| write.0 != atom_buffer),
        "dynamic connectivity must not upload stable atom records"
    );
}
