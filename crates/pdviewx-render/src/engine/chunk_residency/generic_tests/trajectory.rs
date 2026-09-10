use super::*;

#[test]
fn paged_atom_relations_resolve_after_gpu_trajectory_interpolation() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let structure_source = structure_fixture(301, 401, 3);
    let start_source = frame_fixture(302, 501, 2, 0.0);
    let end_source = frame_fixture(303, 601, 2, 10.0);
    let structure = request_deliver_upload(
        &mut engine,
        &mut output,
        structure_source.request,
        structure_source.data,
    );
    let start = request_deliver_upload(
        &mut engine,
        &mut output,
        start_source.request,
        start_source.data,
    );
    let end = request_deliver_upload(
        &mut engine,
        &mut output,
        end_source.request,
        end_source.data,
    );
    let representation = ChunkRepresentation::points(2.0, Rgba8::WHITE)
        .unwrap_or_else(|error| panic!("structure style validates: {error}"));
    engine
        .set_structure_chunk_placements(&[StructureChunkPlacement {
            id: ChunkPlacementId::new(81),
            ticket: structure,
            model_to_world: Mat4::IDENTITY,
            representation,
        }])
        .unwrap_or_else(|error| panic!("structure placement must fit: {error}"));
    engine
        .set_trajectory_chunk_windows(&[TrajectoryChunkWindow::new(structure, start, end, 0.25)
            .unwrap_or_else(|error| panic!("trajectory window validates: {error}"))])
        .unwrap_or_else(|error| panic!("trajectory window must fit: {error}"));

    let atom = ChunkEntityRef::new(
        structure.key.dataset,
        structure.key.chunk,
        ChunkPlacementId::new(81),
        LogicalRow::new(0),
        ChunkSpatialKind::Atom,
    );
    let payload = RelationChunkPayload::new(Arc::from([
        PagedRelation {
            start: PagedSpatialAnchor::Entity(atom),
            end: PagedSpatialAnchor::World(Vec3::ZERO),
        },
        PagedRelation {
            start: PagedSpatialAnchor::World(Vec3::ONE),
            end: PagedSpatialAnchor::Entity(atom),
        },
    ]))
    .unwrap_or_else(|error| panic!("atom relation payload validates: {error}"));
    let (request, data) = fixture(
        DatasetId::new(901),
        902,
        PayloadKind::RelationBatch,
        ChunkPayload::RelationBatch(payload),
    );
    let relation_ticket = request_deliver_upload(&mut engine, &mut output, request, data);
    let relation = RelationChunkPlacement::new(
        ChunkPlacementId::new(82),
        relation_ticket,
        RelationStyle::default(),
    )
    .unwrap_or_else(|error| panic!("relation placement validates: {error}"));
    engine
        .set_relation_chunk_placements(std::slice::from_ref(&relation))
        .unwrap_or_else(|error| panic!("relation placement must fit: {error}"));

    let scene = pdviewx_core::Scene::new();
    engine
        .render(&scene, &crate::engine::tests::camera())
        .unwrap_or_else(|error| panic!("trajectory relation must render: {error}"));
    assert_compute_precedes(
        &engine,
        "paged trajectory interpolation",
        "dynamic relation resolution",
    );
    assert_eq!(
        engine.relation_chunk_placement_status(relation.id()),
        ChunkPlacementStatus::Resident
    );

    let endpoint_writes = relation_write_count(&engine);
    engine
        .set_trajectory_chunk_windows(&[TrajectoryChunkWindow::new(structure, start, end, 0.75)
            .unwrap_or_else(|error| panic!("advanced trajectory window validates: {error}"))])
        .unwrap_or_else(|error| panic!("advanced trajectory window must fit: {error}"));
    engine
        .render(&scene, &crate::engine::tests::camera())
        .unwrap_or_else(|error| panic!("advanced trajectory relation must render: {error}"));
    assert_eq!(relation_write_count(&engine), endpoint_writes);
}
