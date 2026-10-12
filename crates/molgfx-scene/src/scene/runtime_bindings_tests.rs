use crate::{Color, DataSource, RuntimeBindings, Scene, ScenePatch, VolumeBinding, density};
use std::{collections::BTreeMap, sync::Arc};

#[test]
fn restoring_a_scene_without_an_animated_copy_prunes_its_interval_before_publication() {
    let mut scene = Scene::from_structure(&super::tests::structure()).expect("scene");
    let original = scene.snapshot();
    let source = scene.sources().values().next().expect("source").clone();
    let count = source.coordinates().len();
    let copy = scene.add_source(&source).expect("copy");
    scene
        .set_trajectory_pair(
            copy,
            crate::TrajectoryFrame::new(0, 0.0, Arc::from(vec![[0.0; 3]; count])),
            crate::TrajectoryFrame::new(1, 1.0, Arc::from(vec![[1.0; 3]; count])),
            0.5,
        )
        .expect("pair");
    assert_eq!(scene.playback_bindings().count(), 1);
    scene
        .restore_snapshot(&original)
        .expect("restore only the original molecule");
    assert_eq!(scene.playback_bindings().count(), 0);
    assert!(!scene.has_trajectory_pair(copy));
    let revision = scene.revision();
    let archive = scene.capture_runtime().archive().expect("archive");
    let reopened = archive
        .restore(scene.sources().clone())
        .expect("reopen without invalid playback");
    assert_eq!(reopened.spec(), scene.spec());
    assert_eq!(scene.revision(), revision);
}

#[test]
fn imperative_intervals_restore_shared_owners_without_authoring_a_trajectory() {
    let mut scene = Scene::from_structure(&super::tests::structure()).expect("structure");
    let structure = crate::StructureId::new(1);
    let count = scene
        .molecular_pose(structure)
        .expect("pose")
        .positions()
        .len();
    let start = crate::TrajectoryFrame::new(7, 7.0, Arc::from(vec![[0.0; 3]; count]));
    let end = crate::TrajectoryFrame::new(8, 8.0, Arc::from(vec![[2.0; 3]; count]));
    scene
        .set_trajectory_pair(structure, start.clone(), end.clone(), 7.25)
        .expect("interval");
    let spec = scene.to_spec();
    let bindings = RuntimeBindings {
        playback: scene.playback_bindings().collect(),
        ..RuntimeBindings::default()
    };
    let restore = |bindings| {
        Scene::from_spec_with_bindings(
            spec.clone(),
            scene.sources().clone(),
            BTreeMap::new(),
            bindings,
        )
    };
    let mut restored = restore(bindings.clone()).expect("restore");
    let pose = restored.molecular_pose(structure).expect("restored pose");
    assert_eq!(pose.positions().as_ptr(), start.positions().as_ptr());
    assert_eq!(
        pose,
        scene.molecular_pose(structure).expect("original pose")
    );
    assert_eq!(restored.spec(), &spec);
    restored
        .set_trajectory_time(structure, 7.75)
        .expect("sample");
    assert_eq!(
        restored
            .playback_bindings()
            .next()
            .expect("pair")
            .sample_seconds
            .to_bits(),
        7.75f32.to_bits()
    );
    restored
        .apply(&ScenePatch {
            base_revision: spec.revision,
            operations: vec![crate::PatchOperation::SetCamera { camera: None }],
        })
        .expect("next authored edit");
    assert_eq!(restored.revision(), spec.revision + 1);
    assert_eq!(
        restored
            .playback_bindings()
            .next()
            .expect("pair")
            .end
            .positions()
            .as_ptr(),
        end.positions().as_ptr()
    );
    let mut duplicate = bindings.clone();
    duplicate.playback.push(duplicate.playback[0].clone());
    assert!(restore(duplicate).is_err());
    let mut invalid = bindings.clone();
    invalid.playback[0].sample_seconds = 9.0;
    assert!(restore(invalid).is_err());
    let mut missing = bindings;
    missing.playback[0].structure = crate::StructureId::new(99);
    assert!(restore(missing).is_err());
    assert_eq!(scene.spec(), &spec);
}

#[test]
fn restoring_bulk_data_preserves_the_authored_revision_and_shared_storage() {
    let mut scene = Scene::from_structure(&super::tests::structure()).expect("structure");
    let source = DataSource::new("restored-grid");
    let values: Arc<[f32]> = Arc::from([0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
    let id = scene
        .add_volume_with_binding(
            density::volume(source.clone(), [2; 3]).isosurface(0.5, Color::rgb(0, 80, 255), 1.0),
            VolumeBinding::new(source, [2; 3], Arc::clone(&values)),
        )
        .expect("authored binding");
    let spec = scene.to_spec();
    let bindings = RuntimeBindings {
        volumes: scene
            .bound_volumes()
            .map(|(_, binding)| binding.clone())
            .collect(),
        ..RuntimeBindings::default()
    };
    let mut restored = Scene::from_spec_with_bindings(
        spec.clone(),
        scene.sources().clone(),
        BTreeMap::new(),
        bindings,
    )
    .expect("restore");
    assert_eq!(restored.spec(), &spec);
    assert!(restored.unresolved_overlays().is_empty());
    assert_eq!(restored.overlay_handles().volumes, 1);
    let (bound_id, binding) = restored.bound_volumes().next().expect("borrowed asset");
    assert_eq!(bound_id, id);
    assert_eq!(binding.values().as_ptr(), values.as_ptr());
    let patch = ScenePatch {
        base_revision: spec.revision,
        operations: vec![crate::PatchOperation::RemoveVolume { id }],
    };
    restored.apply(&patch).expect("next authored patch");
    assert_eq!(restored.revision(), spec.revision + 1);
}

#[test]
fn a_volume_only_scene_restores_its_asset_and_refuses_malformed_columns() {
    let mut scene = Scene::empty();
    let source = DataSource::new("standalone-map");
    scene
        .add(density::volume(source.clone(), [2; 3]).isosurface(0.5, Color::rgb(0, 80, 255), 1.0))
        .expect("descriptor");
    let restore = |values| {
        Scene::from_spec_with_bindings(
            scene.to_spec(),
            BTreeMap::new(),
            BTreeMap::new(),
            RuntimeBindings {
                volumes: vec![VolumeBinding::new(source.clone(), [2; 3], values)],
                ..RuntimeBindings::default()
            },
        )
    };
    assert!(restore(Arc::from([0.0; 7])).is_err());
    let restored = restore(Arc::from([0.0; 8])).expect("grid without a molecule");
    assert_eq!(restored.revision(), scene.revision());
    assert_eq!(restored.overlay_handles().volumes, 1);
}

#[test]
fn restoring_scientific_geometry_and_shared_frames_preserves_current_stale_and_reversed_samples() {
    let (source, _) = molframe::read_bytes(
        b"HETATM    1  O   HOH A   1       0.000   0.000   0.000  1.00 10.00           O\nEND\n"
            .to_vec(),
        Some("restored-pose.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source");
    let mut scene = Scene::from_structure(&source).expect("scene");
    let structure = crate::StructureId::new(1);
    let data_source = DataSource::new("retained-pose-interval");
    scene
        .add(crate::trajectory::trajectory(
            structure,
            data_source.clone(),
            2,
        ))
        .expect("trajectory");
    let start = crate::TrajectoryFrame::new(0, 0.0, Arc::from([[0.0; 3]]));
    let end = crate::TrajectoryFrame::new(1, 1.0, Arc::from([[2.0, 0.0, 0.0]]));
    let pair = crate::TrajectoryBinding::new(data_source, start.clone(), end).sample_seconds(0.25);
    scene
        .bind_trajectory(pair.clone())
        .expect("resident frames");
    let pose = pair
        .capture_pose(&scene.sources()[&structure])
        .expect("shared pose");
    assert_eq!(pose.positions().as_ptr(), start.positions().as_ptr());
    let identity = scene
        .add(crate::rep::surf(
            "all",
            DataSource::new("restored-sampled-surface"),
        ))
        .expect("surface");
    scene
        .bind_molecular_surface_data_at_pose(
            identity,
            crate::MolecularSurfaceData {
                method: crate::MolecularSurfaceMethod::Surf,
                controls: crate::MolecularSurfaceControls::default(),
                mesh: molgfx_core::MolecularSurfaceDescription {
                    vertices: vec![[0.5, 0.0, 0.0], [1.5, 0.0, 0.0], [0.5, 1.0, 0.0]],
                    normals: vec![[0.0, 0.0, 1.0]; 3],
                    faces: vec![[0, 1, 2]],
                    atom_rows: vec![0; 3],
                    discarded_degenerate_faces: 0,
                },
            },
            &pose,
        )
        .expect("sampled geometry");
    let restore = |scene: &Scene| {
        Scene::from_spec_with_bindings(
            scene.to_spec(),
            scene.sources().clone(),
            BTreeMap::new(),
            RuntimeBindings {
                trajectories: scene
                    .bound_trajectories()
                    .map(|(_, binding)| binding.clone())
                    .collect(),
                molecular_surfaces: scene
                    .bound_molecular_surfaces()
                    .map(|(_, binding)| binding.clone())
                    .collect(),
                ..RuntimeBindings::default()
            },
        )
        .expect("restore")
    };
    let mut restored = restore(&scene);
    assert_eq!(restored.revision(), scene.revision());
    assert!(
        restored
            .molecular_surface_is_current(identity)
            .expect("current")
    );
    restored
        .set_trajectory_time(structure, 0.5)
        .expect("advance");
    assert!(
        !restored
            .molecular_surface_is_current(identity)
            .expect("stale")
    );
    let mut stale_snapshot = restore(&restored);
    assert!(
        !stale_snapshot
            .molecular_surface_is_current(identity)
            .expect("stale restored")
    );
    stale_snapshot
        .set_trajectory_time(structure, 0.25)
        .expect("reverse");
    assert!(
        stale_snapshot
            .molecular_surface_is_current(identity)
            .expect("original owners")
    );
    assert_eq!(stale_snapshot.revision(), scene.revision());
}
