use super::SceneArchive;
use crate::{Color, DataSource, Scene, StructureId, TrajectoryFrame, VolumeBinding, density};
use std::{collections::BTreeMap, sync::Arc};

#[test]
fn archives_encode_shared_grids_once_and_refuse_contradictory_duplicate_assets() {
    let mut scene = Scene::empty();
    let source = DataSource::new("shared-grid");
    let grid = density::volume(source.clone(), [2; 3]).isosurface(0.5, Color::rgb(0, 80, 255), 1.0);
    scene
        .add_volume_with_binding(
            grid.clone(),
            VolumeBinding::new(source, [2; 3], Arc::from([0.0; 8])),
        )
        .expect("grid");
    let alias = scene.add(grid).expect("second presentation");
    let archive = scene.capture_runtime().archive().expect("archive");
    assert_eq!(archive.volumes.len(), 1);
    let decoded = SceneArchive::from_json(&archive.to_json().expect("encoding")).expect("read");
    let restored = decoded.restore(BTreeMap::new()).expect("shared grid");
    assert_eq!(scene.spec(), restored.spec());
    let grids: Vec<_> = restored.bound_volumes().collect();
    assert_eq!(grids.len(), 2);
    assert_eq!(grids[0].1.values().as_ptr(), grids[1].1.values().as_ptr());
    let mut contradictory = archive;
    let different = scene
        .spec()
        .volumes
        .keys()
        .copied()
        .find(|id| *id != alias)
        .expect("other id");
    contradictory.volumes.insert(different, vec![1.0; 8]);
    assert!(contradictory.restore(BTreeMap::new()).is_err());
}

#[test]
fn named_captures_retain_distinct_inputs_under_the_same_property_name_and_preserve_missing_value_bits()
 {
    let (molecule, _) = moving_surface();
    let id = StructureId::new(1);
    let source = molecule.sources()[&id].clone();
    let mut old = Scene::from_source(source.clone()).expect("old scene");
    let missing = f32::from_bits(0x7fc0_0037);
    old.bind_property(
        crate::ScalarPropertyBinding::new(
            id,
            "signal",
            DataSource::new("old-input"),
            Arc::from([missing]),
        )
        .domain([0.0, 3.0]),
    )
    .expect("old input");
    let mut current = Scene::from_source(source).expect("current scene");
    current
        .bind_property(
            crate::ScalarPropertyBinding::new(
                id,
                "signal",
                DataSource::new("new-input"),
                Arc::from([2.0]),
            )
            .domain([0.0, 3.0]),
        )
        .expect("new input");
    let mut capture = current.capture_runtime();
    capture.retained = vec![old.capture_runtime(), old.capture_runtime()];
    let archive = capture.archive().expect("both inputs");
    assert_eq!(archive.properties["signal"], vec![2.0f32.to_bits()]);
    assert_eq!(
        archive.retained[0].properties["signal"],
        vec![missing.to_bits()]
    );
    assert!(archive.retained[1].properties.is_empty());
    let mut restored = SceneArchive::from_json(&archive.to_json().expect("encoding"))
        .expect("decoding")
        .restore(current.sources().clone())
        .expect("restore both inputs");
    assert_eq!(restored.spec(), current.spec());
    let again = restored
        .capture_runtime_with_snapshots([&old.snapshot()])
        .archive()
        .expect("capture retained input again");
    assert_eq!(
        again.retained[0].properties["signal"],
        vec![missing.to_bits()]
    );
    restored
        .restore_snapshot(&old.snapshot())
        .expect("old view");
    assert_eq!(
        restored
            .capture_runtime()
            .archive()
            .expect("old input")
            .properties["signal"],
        vec![missing.to_bits()]
    );
    restored
        .restore_snapshot(&current.snapshot())
        .expect("current view");
    assert_eq!(
        restored
            .capture_runtime()
            .archive()
            .expect("current input")
            .properties["signal"],
        vec![2.0f32.to_bits()]
    );
}

#[test]
fn one_scientific_property_column_supports_distinct_saved_display_domains_without_duplicate_data() {
    let (molecule, _) = moving_surface();
    let id = StructureId::new(1);
    let source = molecule.sources()[&id].clone();
    let values: Arc<[f32]> = Arc::from([1.0]);
    let mut old = Scene::from_source(source.clone()).expect("old");
    old.bind_property(
        crate::ScalarPropertyBinding::new(
            id,
            "signal",
            DataSource::new("shared-input"),
            values.clone(),
        )
        .domain([0.0, 2.0]),
    )
    .expect("old legend");
    let mut current = Scene::from_source(source).expect("current");
    current
        .bind_property(
            crate::ScalarPropertyBinding::new(
                id,
                "signal",
                DataSource::new("shared-input"),
                values,
            )
            .domain([0.0, 3.0]),
        )
        .expect("current legend");
    let mut capture = current.capture_runtime();
    capture.retained = vec![old.capture_runtime()];
    let archive = capture.archive().expect("shared column");
    assert_eq!(archive.properties.len(), 1);
    assert!(archive.retained[0].properties.is_empty());
    let mut restored = archive
        .restore(current.sources().clone())
        .expect("both legends");
    restored
        .restore_snapshot(&old.snapshot())
        .expect("old legend");
    assert_eq!(
        restored.spec().properties["signal"]
            .domain
            .map(f32::to_bits),
        [0.0f32, 2.0].map(f32::to_bits)
    );
    restored
        .restore_snapshot(&current.snapshot())
        .expect("current legend");
    assert_eq!(
        restored.spec().properties["signal"]
            .domain
            .map(f32::to_bits),
        [0.0f32, 3.0].map(f32::to_bits)
    );
}

fn moving_surface() -> (Scene, crate::RepresentationId) {
    let structure = molframe::read_bytes(
        b"HETATM    1  O   HOH A   1       0.000   0.000   0.000  1.00 10.00           O\nEND\n"
            .to_vec(),
        Some("archive.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source")
    .0;
    let mut scene = Scene::from_structure(&structure).expect("scene");
    let id = StructureId::new(1);
    scene
        .set_trajectory_pair(
            id,
            TrajectoryFrame::new(0, 0.0, Arc::from([[0.0; 3]])),
            TrajectoryFrame::new(1, 1.0, Arc::from([[2.0, 0.0, 0.0]])),
            0.25,
        )
        .expect("sample");
    let pose = scene.molecular_pose(id).expect("pose");
    let surface = scene
        .add(crate::rep::surf("all", DataSource::new("archive-surf")))
        .expect("descriptor");
    scene
        .bind_molecular_surface_data_at_pose(
            surface,
            crate::MolecularSurfaceData {
                method: crate::MolecularSurfaceMethod::Surf,
                controls: crate::MolecularSurfaceControls::default(),
                mesh: crate::source::MolecularSurfaceDescription {
                    vertices: vec![[0.5, 0.0, 0.0], [1.5, 0.0, 0.0], [0.5, 1.0, 0.0]],
                    normals: vec![[0.0, 0.0, 1.0]; 3],
                    faces: vec![[0, 1, 2]],
                    atom_rows: vec![0; 3],
                    discarded_degenerate_faces: 0,
                },
            },
            &pose,
        )
        .expect("scientific surface");
    (scene, surface)
}

#[test]
fn stale_surfaces_reopen_with_their_original_shared_interval_and_become_current_when_reversed() {
    let (mut scene, surface) = moving_surface();
    scene
        .set_trajectory_time(StructureId::new(1), 0.75)
        .expect("advance");
    let capture = scene.capture_runtime();
    let archive = capture.archive().expect("archive");
    assert_eq!(archive.frames.len(), 2);
    let encoded = archive.to_json().expect("encoded");
    assert_eq!(
        capture
            .archive()
            .expect("repeat")
            .to_json()
            .expect("repeat encoding"),
        encoded
    );
    let mut restored = SceneArchive::from_json(&encoded)
        .expect("decode")
        .restore(scene.sources().clone())
        .expect("restore");
    assert_eq!(scene.spec(), restored.spec());
    assert!(
        !restored
            .molecular_surface_is_current(surface)
            .expect("stale")
    );
    let playback = restored.playback_bindings().next().expect("playback");
    let bound = restored
        .bound_molecular_surfaces()
        .next()
        .expect("surface")
        .1;
    let input = bound
        .geometry()
        .pose()
        .expect("original sample")
        .trajectory()
        .expect("original interval");
    assert_eq!(
        playback.start.positions().as_ptr(),
        input.start().positions().as_ptr()
    );
    assert_eq!(
        playback.end.positions().as_ptr(),
        input.end().positions().as_ptr()
    );
    restored
        .set_trajectory_time(StructureId::new(1), 0.25)
        .expect("reverse");
    assert!(
        restored
            .molecular_surface_is_current(surface)
            .expect("original sample")
    );
    assert_eq!(scene.revision(), restored.revision());
}

#[test]
fn malformed_archives_refuse_missing_frames_invalid_vertices_and_extra_invalid_columns() {
    let (scene, _) = moving_surface();
    let archive = scene.capture_runtime().archive().expect("archive");
    let mut absent = archive.clone();
    absent.frames.clear();
    assert!(absent.restore(scene.sources().clone()).is_err());
    let mut malformed = archive.clone();
    malformed
        .surfaces
        .values_mut()
        .next()
        .expect("mesh")
        .data
        .mesh
        .faces[0][0] = 3;
    assert!(malformed.restore(scene.sources().clone()).is_err());
    let mut invalid = archive.clone();
    invalid.frames.push(super::types::Frame {
        index: 99,
        time: 99.0,
        positions: vec![[f32::NAN; 3]],
    });
    assert!(invalid.restore(scene.sources().clone()).is_err());
    assert!(archive.restore(BTreeMap::new()).is_err());
}

#[test]
fn cancelled_capture_and_restoration_return_no_partial_archive_or_scene() {
    let (scene, _) = moving_surface();
    let archive = scene.capture_runtime().archive().expect("archive");
    let token = molframe::CancellationToken::new();
    token.cancel();
    assert!(
        scene
            .capture_runtime()
            .archive_with_cancellation(&token)
            .is_err()
    );
    assert!(
        archive
            .restore_with_cancellation(scene.sources().clone(), &token)
            .is_err()
    );
    let token = molframe::CancellationToken::new();
    let values = (0..8192).inspect(|index| {
        if *index == 2048 {
            token.cancel();
        }
    });
    assert!(super::control::collect(values, &token).is_err());
}

#[test]
fn oriented_candidates_reopen_sharing_playback_frames_and_keep_original_atom_addresses() {
    let pdb = b"ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 20.00           N\nATOM      2  H   ALA A   1       1.000   0.000   0.000  1.00 20.00           H\nATOM      3  O   ALA A   2       2.800   0.000   0.000  1.00 20.00           O\nEND\n";
    let structure = molframe::read_bytes(
        pdb.to_vec(),
        Some("bonds.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source")
    .0;
    let mut scene = Scene::from_structure(&structure).expect("scene");
    let id = StructureId::new(1);
    scene
        .set_trajectory_pair(
            id,
            TrajectoryFrame::new(
                0,
                0.0,
                Arc::from([[0.0; 3], [1.0, 0.0, 0.0], [6.0, 0.0, 0.0]]),
            ),
            TrajectoryFrame::new(
                1,
                1.0,
                Arc::from([[0.0; 3], [1.0, 0.0, 0.0], [-6.0, 0.0, 0.0]]),
            ),
            0.25,
        )
        .expect("sample");
    let input = scene.molecular_pose(id).expect("input");
    let source = DataSource::new("archive-oriented-candidates");
    scene
        .add(
            crate::HydrogenBonds::new(source.clone(), id)
                .geometry(crate::HydrogenBondGeometry {
                    maximum_endpoint_distance: 3.5,
                    minimum_angle_degrees: 120.0,
                    maximum_pivot_end_distance: None,
                })
                .interaction(),
        )
        .expect("descriptor");
    scene
        .bind_hydrogen_bonds(
            crate::HydrogenBondBinding::candidates(source, Arc::from([[0, 1, 2]]), 3.5)
                .at_pose(input),
        )
        .expect("chemical input");
    scene.set_trajectory_time(id, 0.75).expect("advance");
    let archive = scene.capture_runtime().archive().expect("archive");
    assert_eq!(archive.frames.len(), 2);
    let mut restored = SceneArchive::from_json(&archive.to_json().expect("encoding"))
        .expect("decoding")
        .restore(scene.sources().clone())
        .expect("restoration");
    let binding = restored
        .bound_hydrogen_bonds()
        .next()
        .expect("oriented input")
        .1;
    assert_eq!(binding.triples().collect::<Vec<_>>(), vec![[0, 1, 2]]);
    assert_eq!(binding.candidate_distance(), Some(3.5));
    let playback = restored.playback_bindings().next().expect("playback");
    let pose = binding.pose().expect("original interval");
    assert_eq!(
        pose.positions().as_ptr(),
        playback.start.positions().as_ptr()
    );
    assert_eq!(
        pose.end_positions().expect("end").as_ptr(),
        playback.end.positions().as_ptr()
    );
    assert_eq!(
        pose.trajectory()
            .expect("original time")
            .sample_seconds()
            .to_bits(),
        0.25f64.to_bits()
    );
    assert_eq!(playback.sample_seconds.to_bits(), 0.75f64.to_bits());
    restored.set_trajectory_time(id, 0.25).expect("reverse");
    assert_eq!(restored.spec(), scene.spec());
}

#[test]
fn named_captures_retain_removed_scientific_assets_without_repeating_meshes_or_frame_columns() {
    let (mut scene, surface) = moving_surface();
    let snapshot = crate::snapshot::SceneSnapshot::capture(scene.spec());
    scene
        .apply(&crate::ScenePatch {
            base_revision: scene.revision(),
            operations: vec![crate::PatchOperation::RemoveRepresentation { id: surface }],
        })
        .expect("remove visible surface");
    scene
        .set_trajectory_time(StructureId::new(1), 0.75)
        .expect("advance");
    let archive = scene
        .capture_runtime_with_snapshots([&snapshot, &snapshot])
        .archive()
        .expect("capture retained assets");
    assert_eq!(archive.frames.len(), 2);
    assert!(archive.surfaces.is_empty());
    assert_eq!(archive.retained[0].surfaces.len(), 1);
    assert!(archive.retained[1].surfaces.is_empty());
    assert!(
        archive
            .retained
            .iter()
            .all(|archive| archive.frames.is_empty())
    );
    let mut restored = SceneArchive::from_json(&archive.to_json().expect("encode"))
        .expect("decode")
        .restore(scene.sources().clone())
        .expect("restore all named assets");
    assert_eq!(restored.spec(), scene.spec());
    restored
        .apply(&crate::ScenePatch {
            base_revision: restored.revision(),
            operations: vec![crate::PatchOperation::RestoreSnapshot(Box::new(snapshot))],
        })
        .expect("restore named surface");
    assert!(restored.unresolved_representations().is_empty());
    assert!(
        !restored
            .molecular_surface_is_current(surface)
            .expect("stale")
    );
    restored
        .set_trajectory_time(StructureId::new(1), 0.25)
        .expect("original sample");
    assert!(
        restored
            .molecular_surface_is_current(surface)
            .expect("retained owners match")
    );
}
