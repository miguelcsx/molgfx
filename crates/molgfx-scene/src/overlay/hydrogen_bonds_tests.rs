use crate::{DataSource, HydrogenBondBinding, HydrogenBondGeometry, HydrogenBonds, Scene};
use molframe::AtomIndex;
use molframe::analysis::{HydrogenBond, HydrogenBondTable};
use molgfx_core::{AnchorLayout, RelationPattern};
use std::sync::Arc;

fn scene() -> Scene {
    let pdb = b"ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 20.00           N\nATOM      2  H   ALA A   1       1.000   0.000   0.000  1.00 20.00           H\nATOM      3  O   ALA A   2       2.800   0.000   0.000  1.00 20.00           O\nEND\n";
    let (structure, _) = molframe::read_bytes(
        pdb.to_vec(),
        Some("bond.pdb"),
        &molframe::ReadOptions::new(),
    )
    .unwrap();
    Scene::from_structure(&structure).unwrap()
}

fn table(acceptor: u32) -> Arc<HydrogenBondTable> {
    Arc::new(
        [HydrogenBond {
            donor: AtomIndex::new(0),
            hydrogen: AtomIndex::new(1),
            acceptor: AtomIndex::new(acceptor),
            donor_acceptor_distance: 2.8,
            hydrogen_acceptor_distance: 1.8,
            angle_degrees: 180.0,
        }]
        .into_iter()
        .collect(),
    )
}

#[test]
fn a_chemical_table_and_its_descriptor_commit_once_and_invalid_rows_do_not_poison_retry() {
    let mut scene = scene();
    let structure = *scene.spec().structures.keys().next().unwrap();
    let source = DataSource::new("atomic-hydrogen-bonds");
    let before = scene.to_spec();
    let patch = crate::ScenePatch {
        base_revision: before.revision,
        operations: vec![crate::PatchOperation::AddInteraction {
            id: crate::InteractionId::new(1),
            interaction: HydrogenBonds::new(source.clone(), structure).interaction(),
        }],
    };
    assert!(
        scene
            .apply_with_hydrogen_bonds(
                &patch,
                HydrogenBondBinding::from_molframe(source.clone(), table(3)),
            )
            .is_err()
    );
    assert_eq!(scene.spec(), &before);
    assert!(scene.bound_hydrogen_bonds().next().is_none());
    scene
        .apply_with_hydrogen_bonds(&patch, HydrogenBondBinding::from_molframe(source, table(2)))
        .expect("corrected result");
    assert_eq!(scene.revision(), before.revision + 1);
    assert_eq!(scene.overlay_handles().interactions, 1);
    assert!(scene.unresolved_overlays().is_empty());
    let committed = scene.to_spec();
    let source = DataSource::new("orphan-chemical-input");
    assert!(
        scene
            .apply_with_hydrogen_bonds(
                &crate::ScenePatch::empty(scene.revision()),
                HydrogenBondBinding::from_molframe(source, table(2)),
            )
            .is_err()
    );
    assert_eq!(scene.spec(), &committed);
}

#[test]
fn hydrogen_bonds_use_dynamic_atom_anchors_and_keep_the_upstream_row_identity() {
    let mut scene = scene();
    let structure = *scene.spec().structures.keys().next().unwrap();
    let source = DataSource::new("explicit-hydrogen-analysis");
    scene
        .add(
            HydrogenBonds::new(source.clone(), structure)
                .selection("not element H")
                .interaction(),
        )
        .unwrap();
    assert_eq!(scene.resolved().relation_batches().count(), 0);
    scene
        .bind_hydrogen_bonds(HydrogenBondBinding::from_molframe(source, table(2)))
        .unwrap();
    assert!(scene.unresolved_overlays().is_empty());
    assert_eq!(scene.overlay_handles().interactions, 1);
    let portable = serde_json::to_string(scene.spec()).unwrap();
    let roundtrip: crate::SceneSpec = serde_json::from_str(&portable).unwrap();
    assert_eq!(&roundtrip, scene.spec());
    let (_, batch) = scene.resolved().relation_batches().next().unwrap();
    assert_eq!(batch.relations().len(), 1);
    let relation = batch.relations()[0];
    assert_eq!(relation.start.layout(), AnchorLayout::Atom);
    assert_eq!(relation.end.layout(), AnchorLayout::Atom);
    assert!(relation.start.is_dynamic() && relation.end.is_dynamic());
    assert_eq!(batch.style().pattern, RelationPattern::Dotted);
    assert_eq!(batch.source_rows().key(0), Some(0));
}

#[test]
fn an_out_of_bounds_analysis_table_fails_without_installing_the_binding() {
    let mut scene = scene();
    let structure = *scene.spec().structures.keys().next().unwrap();
    let source = DataSource::new("analysis");
    scene
        .add(HydrogenBonds::new(source.clone(), structure).interaction())
        .unwrap();
    let before = scene.to_spec();
    assert!(
        scene
            .bind_hydrogen_bonds(HydrogenBondBinding::from_molframe(source.clone(), table(3)))
            .is_err()
    );
    assert_eq!(scene.spec(), &before);
    assert_eq!(scene.resolved().relation_batches().count(), 0);
    scene
        .bind_hydrogen_bonds(HydrogenBondBinding::from_molframe(source, table(2)))
        .unwrap();
    assert_eq!(scene.resolved().relation_batches().count(), 1);
}

#[test]
fn selection_requires_both_heavy_atoms_but_not_the_explicit_hydrogen() {
    let mut scene = scene();
    let structure = *scene.spec().structures.keys().next().unwrap();
    let source = DataSource::new("analysis");
    scene
        .add(
            HydrogenBonds::new(source.clone(), structure)
                .selection("element N")
                .interaction(),
        )
        .unwrap();
    scene
        .bind_hydrogen_bonds(HydrogenBondBinding::from_molframe(source, table(2)))
        .unwrap();
    assert_eq!(scene.resolved().relation_batches().count(), 0);
}

#[test]
fn candidate_geometry_is_portable_and_a_too_small_envelope_cannot_poison_binding_retry() {
    let mut scene = scene();
    let structure = *scene.spec().structures.keys().next().unwrap();
    let source = DataSource::new("motion-candidates");
    let geometry = HydrogenBondGeometry {
        maximum_endpoint_distance: 3.5,
        minimum_angle_degrees: 150.0,
        maximum_pivot_end_distance: Some(2.5),
    };
    scene
        .add(
            HydrogenBonds::new(source.clone(), structure)
                .geometry(geometry)
                .interaction(),
        )
        .unwrap();
    let before = scene.to_spec();
    assert!(
        scene
            .bind_hydrogen_bonds(HydrogenBondBinding::candidates(
                source.clone(),
                Arc::from([[0, 1, 2]]),
                2.0
            ))
            .is_err()
    );
    assert_eq!(scene.spec(), &before);
    assert_eq!(scene.resolved().relation_batches().count(), 0);
    scene
        .bind_hydrogen_bonds(HydrogenBondBinding::candidates(
            source,
            Arc::from([[0, 1, 2]]),
            3.5,
        ))
        .unwrap();
    let (_, batch) = scene.resolved().relation_batches().next().unwrap();
    let gate = batch.oriented_geometry().unwrap()[0];
    assert_eq!(gate.pivot.row(), 1);
    assert_eq!(gate.geometry, geometry);
    let roundtrip: crate::SceneSpec =
        serde_json::from_str(&serde_json::to_string(scene.spec()).unwrap()).unwrap();
    assert_eq!(roundtrip, scene.to_spec());
}

#[test]
fn candidates_cannot_be_presented_as_already_classified_bonds() {
    let mut scene = scene();
    let structure = *scene.spec().structures.keys().next().unwrap();
    let source = DataSource::new("unfiltered-candidates");
    scene
        .add(HydrogenBonds::new(source.clone(), structure).interaction())
        .unwrap();
    assert!(
        scene
            .bind_hydrogen_bonds(HydrogenBondBinding::candidates(
                source,
                Arc::from([[0, 1, 2]]),
                3.5
            ))
            .is_err()
    );
    assert_eq!(scene.resolved().relation_batches().count(), 0);
}

#[test]
fn repeated_candidates_share_the_original_input_across_time_but_reject_changed_owners_or_rows() {
    let mut scene = scene();
    let structure = *scene.spec().structures.keys().next().unwrap();
    let source = DataSource::new("resident-candidates");
    let start = crate::TrajectoryFrame::new(
        7,
        7.0,
        Arc::from([[0.0; 3], [1.0, 0.0, 0.0], [6.0, 0.0, 0.0]]),
    );
    let end = crate::TrajectoryFrame::new(
        8,
        8.0,
        Arc::from([[0.0; 3], [1.0, 0.0, 0.0], [-6.0, 0.0, 0.0]]),
    );
    scene
        .set_trajectory_pair(structure, start.clone(), end.clone(), 7.25)
        .expect("pair");
    scene
        .add(
            HydrogenBonds::new(source.clone(), structure)
                .geometry(HydrogenBondGeometry {
                    maximum_endpoint_distance: 3.5,
                    minimum_angle_degrees: 120.0,
                    maximum_pivot_end_distance: None,
                })
                .interaction(),
        )
        .expect("descriptor");
    let original = scene.molecular_pose(structure).expect("original sample");
    let binding = |rows| HydrogenBondBinding::candidates(source.clone(), Arc::from(rows), 3.5);
    scene
        .bind_hydrogen_bonds(binding([[0, 1, 2]]).at_pose(original.clone()))
        .expect("original table");
    scene
        .set_trajectory_time(structure, 7.75)
        .expect("new time");
    scene
        .bind_hydrogen_bonds(binding([[0, 1, 2]]).at_pose(scene.molecular_pose(structure).unwrap()))
        .expect("same interval");
    assert_eq!(scene.bound_hydrogen_bonds().count(), 1);
    assert_eq!(
        scene.bound_hydrogen_bonds().next().unwrap().1.pose(),
        Some(&original)
    );
    let before = scene.to_spec();
    assert!(
        scene
            .bind_hydrogen_bonds(binding([[2, 1, 0]]).at_pose(original))
            .is_err()
    );
    assert_eq!(scene.spec(), &before);
    let unrelated = crate::TrajectoryFrame::new(7, 7.0, Arc::from(start.positions().to_vec()));
    scene
        .set_trajectory_pair(structure, unrelated, end, 7.75)
        .expect("different owner");
    let before = scene.to_spec();
    assert!(
        scene
            .bind_hydrogen_bonds(
                binding([[0, 1, 2]]).at_pose(scene.molecular_pose(structure).unwrap())
            )
            .is_err()
    );
    assert_eq!(scene.spec(), &before);
}

#[test]
fn repeated_classified_tables_reject_conflicting_measured_geometry() {
    let mut scene = scene();
    let structure = *scene.spec().structures.keys().next().unwrap();
    let source = DataSource::new("classified-snapshot");
    scene
        .add(HydrogenBonds::new(source.clone(), structure).interaction())
        .expect("descriptor");
    scene
        .bind_hydrogen_bonds(HydrogenBondBinding::from_molframe(source.clone(), table(2)))
        .expect("first table");
    scene
        .bind_hydrogen_bonds(HydrogenBondBinding::from_molframe(source.clone(), table(2)))
        .expect("same complete result");
    let mut row = table(2).row(0).unwrap();
    row.angle_degrees = 150.0;
    let changed = Arc::new([row].into_iter().collect());
    let before = scene.to_spec();
    assert!(
        scene
            .bind_hydrogen_bonds(HydrogenBondBinding::from_molframe(source, changed))
            .is_err()
    );
    assert_eq!(scene.spec(), &before);
}

#[test]
fn obsolete_chemical_input_is_released_only_after_no_retained_history_needs_it() {
    let mut scene = scene();
    let structure = *scene.spec().structures.keys().next().unwrap();
    let source = DataSource::new("retained-chemical-input");
    let id = scene
        .add(HydrogenBonds::new(source.clone(), structure).interaction())
        .expect("descriptor");
    let data = table(2);
    let weak = Arc::downgrade(&data);
    scene
        .bind_hydrogen_bonds(HydrogenBondBinding::from_molframe(source.clone(), data))
        .expect("bound input");
    let patch = crate::ScenePatch {
        base_revision: scene.revision(),
        operations: vec![crate::PatchOperation::RemoveInteraction { id }],
    };
    let inverse = patch.inverse(scene.spec()).expect("undo input");
    scene.apply(&patch).expect("remove descriptor");
    let revision = scene.revision();
    scene.prune_runtime_bindings(
        inverse
            .operations
            .iter()
            .flat_map(crate::PatchOperation::runtime_sources),
    );
    assert!(
        weak.upgrade().is_some(),
        "undo still requires the original shared table"
    );
    scene
        .apply(&crate::ScenePatch {
            base_revision: scene.revision(),
            operations: inverse.operations,
        })
        .expect("undo after retention");
    scene.prune_runtime_bindings([]);
    assert!(
        weak.upgrade().is_some(),
        "current scene always retains its own input"
    );
    scene
        .apply(&crate::ScenePatch {
            base_revision: scene.revision(),
            operations: patch.operations,
        })
        .expect("remove again");
    let revision_after = scene.revision();
    scene.prune_runtime_bindings([]);
    assert!(
        weak.upgrade().is_none(),
        "unreachable scientific tables do not grow indefinitely"
    );
    assert_eq!(scene.revision(), revision_after);
    assert!(revision_after > revision);
}
