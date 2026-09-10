use super::*;

pub(in crate::engine) fn assert_compute_precedes(
    engine: &crate::engine::Engine<crate::testing::MockDevice>,
    before: &'static str,
    after: &'static str,
) {
    let passes = engine
        .device
        .log
        .compute_passes
        .lock()
        .map_or_else(|_| Vec::new(), |value| value.clone());
    let before_index = passes
        .iter()
        .position(|label| *label == before)
        .unwrap_or_else(|| panic!("{before} pass must be recorded"));
    let after_index = passes
        .iter()
        .position(|label| *label == after)
        .unwrap_or_else(|| panic!("{after} pass must be recorded"));
    assert!(before_index < after_index);
}

pub(in crate::engine) fn request_deliver_upload(
    engine: &mut crate::engine::Engine<crate::testing::MockDevice>,
    output: &mut ResidencyOutput,
    request: ResidencyRequest,
    data: ChunkData,
) -> pdviewx_core::ResidencyTicket {
    let ticket = engine
        .request_chunk_into(request, output)
        .unwrap_or_else(|error| panic!("generic request must fit: {error}"));
    engine
        .deliver_chunk_into(ticket, data, output)
        .unwrap_or_else(|error| panic!("generic delivery must fit: {error}"));
    upload(engine, ticket, output);
    complete(engine, output);
    ticket
}

pub(in crate::engine) fn relation_write_count(
    engine: &crate::engine::Engine<crate::testing::MockDevice>,
) -> usize {
    let labels = [
        "base relation glyph table",
        "interaction glyph table",
        "dynamic relation resolver table",
        "interaction glyph indirect arguments",
        "relation cull range",
        "paged relation source model",
        "paged visual instructions",
        "paged visual parameters",
        "visual entity results",
        "visual configuration",
        "fragment visual program",
    ];
    write_count_for_labels(engine, &labels)
}

pub(in crate::engine) fn relation_geometry_write_count(
    engine: &crate::engine::Engine<crate::testing::MockDevice>,
) -> usize {
    write_count_for_labels(
        engine,
        &[
            "base relation glyph table",
            "interaction glyph table",
            "dynamic relation resolver table",
            "interaction glyph indirect arguments",
            "relation cull range",
            "paged relation source model",
        ],
    )
}

pub(in crate::engine) fn write_count_for_labels(
    engine: &crate::engine::Engine<crate::testing::MockDevice>,
    labels: &[&'static str],
) -> usize {
    let ids = engine.device.log.buffers.lock().map_or_else(
        |_| Vec::new(),
        |buffers| {
            buffers
                .iter()
                .filter_map(|(id, label, _)| labels.contains(label).then_some(*id))
                .collect::<Vec<_>>()
        },
    );
    engine.device.log.writes.lock().map_or(0, |writes| {
        writes
            .iter()
            .filter(|(id, _, _, _)| ids.contains(id))
            .count()
    })
}

pub(in crate::engine) fn buffer_id(
    engine: &crate::engine::Engine<crate::testing::MockDevice>,
    label: &'static str,
) -> u32 {
    engine
        .device
        .log
        .buffers
        .lock()
        .map_or(u32::MAX, |buffers| {
            buffers
                .iter()
                .find(|(_, candidate, _)| *candidate == label)
                .map_or(u32::MAX, |(id, _, _)| *id)
        })
}

pub(in crate::engine) fn buffer_count(
    engine: &crate::engine::Engine<crate::testing::MockDevice>,
) -> usize {
    engine
        .device
        .log
        .buffers
        .lock()
        .map_or(0, |value| value.len())
}

pub(in crate::engine) fn indirect_count(
    engine: &crate::engine::Engine<crate::testing::MockDevice>,
    buffer: u32,
) -> usize {
    engine.device.log.indirect_draws.lock().map_or(0, |draws| {
        draws.iter().filter(|(id, _)| *id == buffer).count()
    })
}

pub(in crate::engine) fn paged_write_count(
    engine: &crate::engine::Engine<crate::testing::MockDevice>,
) -> usize {
    let placement = buffer_id(engine, "paged chunk placements");
    let coordinates = buffer_id(engine, "resident structure chunks");
    let clusters = buffer_id(engine, "resident structure chunk clusters");
    engine.device.log.writes.lock().map_or(0, |writes| {
        writes
            .iter()
            .filter(|(id, _, _, _)| [placement, coordinates, clusters].contains(id))
            .count()
    })
}

pub(in crate::engine) fn generic_fixtures() -> Vec<(ResidencyRequest, ChunkData)> {
    let dataset = DatasetId::new(0x1_0000_0007);
    let points = PointChunkPayload::new(Arc::from([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]))
        .unwrap_or_else(|error| panic!("point fixture must be valid: {error}"));
    let transforms = InstanceChunkPayload::new(Arc::from([rigid(Vec3::ZERO), rigid(Vec3::X)]))
        .unwrap_or_else(|error| panic!("instance fixture must be valid: {error}"));
    let attributes = AttributeChunkPayload::new(
        ChunkDomainRef::new(dataset, ChunkId::new(11), ChunkDomainKind::Point),
        AttributeValues::Scalar(Arc::from([1.0, 2.0])),
    )
    .unwrap_or_else(|error| panic!("attribute fixture must be valid: {error}"));
    let point_ref = ChunkEntityRef::new(
        dataset,
        ChunkId::new(11),
        ChunkPlacementId::new(91),
        LogicalRow::new(100),
        ChunkSpatialKind::Point,
    );
    let relations = RelationChunkPayload::new(Arc::from([
        PagedRelation {
            start: PagedSpatialAnchor::World(Vec3::ZERO),
            end: PagedSpatialAnchor::Entity(point_ref),
        },
        PagedRelation {
            start: PagedSpatialAnchor::Entity(point_ref),
            end: PagedSpatialAnchor::World(Vec3::ONE),
        },
    ]))
    .unwrap_or_else(|error| panic!("relation fixture must be valid: {error}"));
    [
        (
            11,
            PayloadKind::PointBatch,
            ChunkPayload::PointBatch(points),
        ),
        (
            12,
            PayloadKind::InstanceBatch,
            ChunkPayload::InstanceBatch(transforms),
        ),
        (
            13,
            PayloadKind::Attribute,
            ChunkPayload::Attribute(attributes),
        ),
        (
            14,
            PayloadKind::RelationBatch,
            ChunkPayload::RelationBatch(relations),
        ),
    ]
    .into_iter()
    .map(|(chunk, kind, payload)| fixture(dataset, chunk, kind, payload))
    .collect()
}

pub(in crate::engine) fn fixture(
    dataset: DatasetId,
    chunk: u64,
    kind: PayloadKind,
    payload: ChunkPayload,
) -> (ResidencyRequest, ChunkData) {
    let chunk = ChunkId::new(chunk);
    let footprint = ChunkFootprint::new(512, 512, 512, 512);
    let descriptor = ChunkDescriptor {
        id: chunk,
        parent: None,
        level: 0,
        rows: ChunkSpan::new(LogicalRow::new(100), 2)
            .unwrap_or_else(|error| panic!("fixture span must be valid: {error}")),
        bounds: ChunkBounds::new([-10.0; 3], [10.0; 3])
            .unwrap_or_else(|error| panic!("fixture bounds must be valid: {error}")),
        payload_kind: kind,
        footprint,
    };
    let catalog = DatasetCatalog::new(dataset, vec![descriptor])
        .unwrap_or_else(|error| panic!("fixture catalog must be valid: {error}"));
    let data = ChunkData::new(&catalog, chunk, payload)
        .unwrap_or_else(|error| panic!("fixture payload must match: {error}"));
    (
        ResidencyRequest {
            key: ResidencyKey {
                dataset,
                chunk,
                detail: ResidencyDetail::Atom,
            },
            footprint,
            class: ResidencyClass::Hot,
            priority: 1,
        },
        data,
    )
}

pub(in crate::engine) fn rigid(translation: Vec3) -> RigidInstance {
    RigidInstance::new(translation, Quat::IDENTITY, 1.0)
        .unwrap_or_else(|error| panic!("fixture transform must be valid: {error}"))
}
