use super::tests::{camera, engine, represented_scene, structure};

#[test]
fn picking_resolves_the_global_atom_from_integer_attachments() {
    let mut engine = engine();
    let scene = represented_scene(2, 1);
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    let pick = match engine.pick(0, 0) {
        Ok(Some(pick)) => pick,
        Ok(None) => panic!("mock readback resolves zero ids"),
        Err(error) => panic!("pick resolves: {error}"),
    };
    let Some((_, placed)) = scene.structures().next() else {
        panic!("scene has structures")
    };
    let super::PickEntity::Structure(entity) = pick.entity else {
        panic!("molecular pick resolves to a global identity")
    };
    assert_eq!(entity.dataset(), placed.dataset_id());
    assert_eq!(entity.chunk(), pdviewx_core::ChunkId::new(0));
    assert_eq!(entity.kind(), pdviewx_core::EntityKind::Atom);
    assert_eq!(entity.row(), pdviewx_core::LogicalRow::new(0));
    assert!(pick.selection.contains(0));
    assert_eq!(pick.selection.count(3), 1);
    let outside = engine.width;
    assert!(matches!(engine.pick(outside, 0), Ok(None)));
}

#[test]
fn recycled_pick_page_rejects_an_older_submission_generation() {
    let mut engine = engine();
    let mut scene = represented_scene(2, 1);
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .capture_pick_submission_for_test()
        .unwrap_or_else(|error| panic!("{error}"));
    let old = pdviewx_core::GpuPickToken::new(0, 0);

    let source = structure();
    let asset = pdviewx_core::StructureAsset::new(pdviewx_core::DatasetId::new(99), &source)
        .unwrap_or_else(|error| panic!("{error}"));
    scene.add_asset(&asset);
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));

    let error = engine
        .resolve_pick_token_for_test(old)
        .expect_err("recycled page generation must be stale");
    assert_eq!(error.code(), "PDVIEWX-E0082");
    assert!(matches!(
        error,
        crate::RenderError::Picking(pdviewx_core::PickingError::StaleGeneration)
    ));
}

#[test]
fn changing_to_a_distinct_scene_rebuilds_equal_revision_pick_pages() {
    let mut engine = engine();
    let first = represented_scene(2, 1);
    let second = represented_scene(2, 1);
    engine
        .render(&first, &camera())
        .unwrap_or_else(|error| panic!("first scene renders: {error}"));
    engine
        .capture_pick_submission_for_test()
        .unwrap_or_else(|error| panic!("first picking table is captured: {error}"));
    let old = pdviewx_core::GpuPickToken::new(0, 0);

    engine
        .render(&second, &camera())
        .unwrap_or_else(|error| panic!("second scene renders: {error}"));

    let error = engine
        .resolve_pick_token_for_test(old)
        .expect_err("a different scene must recycle equal-revision pages");
    assert!(matches!(
        error,
        crate::RenderError::Picking(pdviewx_core::PickingError::StaleGeneration)
    ));
}
