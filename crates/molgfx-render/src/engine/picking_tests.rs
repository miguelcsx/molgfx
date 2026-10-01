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
    assert_eq!(entity.chunk(), molgfx_core::ChunkId::new(0));
    assert_eq!(entity.kind(), molgfx_core::EntityKind::Atom);
    assert_eq!(entity.row(), molgfx_core::LogicalRow::new(0));
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
    let submission = engine
        .capture_pick_submission_for_test()
        .unwrap_or_else(|error| panic!("{error}"));
    let old = molgfx_core::GpuPickToken::new(0, 0);

    let source = structure();
    let asset = molgfx_core::StructureAsset::new(molgfx_core::DatasetId::new(99), &source)
        .unwrap_or_else(|error| panic!("{error}"));
    scene.add_asset(&asset);
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));

    let error = engine
        .resolve_pick_token_against_for_test(old, &submission)
        .expect_err("recycled page generation must be stale");
    assert_eq!(error.code(), "MOLGFX-E0082");
    assert!(matches!(
        error,
        crate::RenderError::Picking(molgfx_core::PickingError::StaleGeneration)
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
    let submission = engine
        .capture_pick_submission_for_test()
        .unwrap_or_else(|error| panic!("first picking table is captured: {error}"));
    let old = molgfx_core::GpuPickToken::new(0, 0);

    engine
        .render(&second, &camera())
        .unwrap_or_else(|error| panic!("second scene renders: {error}"));

    let error = engine
        .resolve_pick_token_against_for_test(old, &submission)
        .expect_err("a different scene must recycle equal-revision pages");
    assert!(matches!(
        error,
        crate::RenderError::Picking(molgfx_core::PickingError::StaleGeneration)
    ));
}

#[test]
fn concurrent_picks_are_bounded_and_each_owns_its_readback() {
    let mut engine = engine();
    let scene = represented_scene(1, 1);
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let mut pending = Vec::new();
    for _ in 0..4 {
        let Some(pick) = engine
            .begin_pick(0, 0)
            .unwrap_or_else(|error| panic!("{error}"))
        else {
            panic!("a drawable pixel begins a pick")
        };
        pending.push(pick);
    }
    let Err(error) = engine.begin_pick(0, 0) else {
        panic!("a fifth in-flight pick is refused, not queued")
    };
    assert!(matches!(
        error,
        crate::RenderError::Picking(molgfx_core::PickingError::InFlightExhausted)
    ));
    drop(pending.remove(0));
    assert!(matches!(engine.begin_pick(0, 0), Ok(Some(_))));
    assert!(matches!(engine.begin_pick(engine.width, 0), Ok(None)));
}
