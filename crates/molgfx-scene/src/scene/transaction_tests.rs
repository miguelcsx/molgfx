use crate::appearance::tests::two_chains;
use crate::{
    AppearanceRuleSpec, Color, Error, PatchError, PatchOperation, RepresentationId, Scene,
    StructureId, color, rep,
};

fn scene() -> Scene {
    match Scene::from_structure(&two_chains()) {
        Ok(scene) => scene,
        Err(error) => panic!("scene builds: {error}"),
    }
}

#[test]
fn structural_and_local_edits_commit_as_one_revision() {
    let mut scene = scene();
    let revision = scene.revision();
    let mut transaction = scene.begin();
    let Ok(cartoon) = transaction.add(rep::cartoon("protein")) else {
        panic!("cartoon stages")
    };
    let Ok(sticks) = transaction.add(rep::ball_and_stick("resname HEM")) else {
        panic!("sticks stage")
    };
    // Later edits see earlier ones: the new representations can be edited.
    transaction.set_opacity(sticks, 0.5);
    assert!(transaction.set_color(cartoon, color::chain()).is_ok());
    assert!(
        transaction
            .add_appearance_rule(AppearanceRuleSpec::new(
                StructureId(1),
                "chain A",
                color::uniform(Color::rgb(255, 0, 0)),
            ))
            .is_ok()
    );
    let Ok(patch) = scene.commit(transaction) else {
        panic!("transaction commits")
    };
    assert_eq!(patch.base_revision, revision);
    assert_eq!(scene.revision(), revision + 1);
    assert_eq!(scene.spec().representations.len(), 2);
    assert_eq!(scene.spec().appearance.len(), 1);
    assert!(matches!(
        patch.operations.first(),
        Some(PatchOperation::AddRepresentation { .. })
    ));
}

#[test]
fn identities_are_allocated_deterministically_and_never_leak() {
    let mut scene = scene();
    let mut abandoned = scene.begin();
    let Ok(first) = abandoned.add(rep::cartoon("protein")) else {
        panic!("stages")
    };
    drop(abandoned);
    let mut failing = scene.begin();
    let Ok(second) = failing.add(rep::cartoon("protein")) else {
        panic!("stages")
    };
    failing.set_opacity(second, 7.0);
    assert!(scene.commit(failing).is_err());
    let mut committed = scene.begin();
    let Ok(third) = committed.add(rep::cartoon("protein")) else {
        panic!("stages")
    };
    assert_eq!(first, second);
    assert_eq!(second, third);
    assert!(scene.commit(committed).is_ok());
    assert!(scene.spec().representations.contains_key(&third));
}

#[test]
fn an_invalid_staged_edit_is_reported_where_it_is_staged() {
    let scene = scene();
    let mut transaction = scene.begin();
    assert!(transaction.remove(RepresentationId(99)).is_err());
    assert!(transaction.set_target(RepresentationId(99), "all").is_err());
    let Ok(id) = transaction.add(rep::cartoon("protein")) else {
        panic!("stages")
    };
    assert!(transaction.set_target(id, "chain (").is_err());
    // Failed stagings leave nothing behind.
    assert_eq!(transaction.operations().len(), 1);
}

#[test]
fn a_transaction_begun_before_another_commit_conflicts() {
    let mut scene = scene();
    let mut stale = scene.begin();
    let Ok(_) = stale.add(rep::cartoon("protein")) else {
        panic!("stages")
    };
    let Ok(_) = scene.add(rep::spacefill("all")) else {
        panic!("adds")
    };
    let before = scene.to_spec();
    assert!(matches!(
        scene.commit(stale),
        Err(Error::Patch(PatchError::RevisionConflict { .. }))
    ));
    assert_eq!(scene.spec(), &before);
}

#[test]
fn a_committed_transaction_inverts_to_the_base_scene() {
    let mut scene = scene();
    let base = scene.to_spec();
    let mut transaction = scene.begin();
    let Ok(id) = transaction.add(rep::cartoon("protein")) else {
        panic!("stages")
    };
    transaction.set_opacity(id, 0.3);
    let Ok(patch) = scene.commit(transaction) else {
        panic!("commits")
    };
    let Ok(inverse) = patch.inverse(&base) else {
        panic!("inverts")
    };
    assert!(scene.apply(&inverse).is_ok());
    assert_eq!(scene.spec().representations, base.representations);
}

#[test]
fn a_multi_structure_scene_requires_an_explicit_structure() {
    let mut scene = scene();
    let Ok(_) = scene.add_structure(&two_chains()) else {
        panic!("second structure binds")
    };
    let mut transaction = scene.begin();
    assert!(transaction.add(rep::cartoon("protein")).is_err());
}
