use super::chunk_residency_tests::{complete, engine, upload};
use super::generic_chunk_residency_tests::{
    buffer_count, fixture, generic_fixtures, relation_geometry_write_count, relation_write_count,
    request_deliver_upload,
};
use super::{
    AttributeChunkWindow, ChunkPlacementId, ChunkPlacementStatus, PointChunkPlacement,
    RelationChunkPlacement,
};
use molgfx_core::{
    AttributeChunkPayload, AttributeValues, ChunkDomainKind, ChunkDomainRef, ChunkPayload,
    ChunkVisualBinding, ChunkVisualDescriptor, PayloadKind, RelationStyle, ResidencyOutput,
    VisualColumnKey, VisualProgramBuilder, VisualStyle,
};
use molgfx_math::{Mat4, Rgba8};
use std::sync::Arc;

#[test]
fn paged_relation_visual_reads_one_native_resident_column() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let Some((relation_request, relation_data)) = generic_fixtures().into_iter().nth(3) else {
        panic!("relation fixture must exist")
    };
    let relation_ticket =
        request_deliver_upload(&mut engine, &mut output, relation_request, relation_data);
    let Some((point_request, point_data)) = generic_fixtures().into_iter().next() else {
        panic!("point fixture must exist")
    };
    let point_ticket = request_deliver_upload(&mut engine, &mut output, point_request, point_data);
    let point = PointChunkPlacement::new(
        ChunkPlacementId::new(91),
        point_ticket,
        Mat4::IDENTITY,
        2.0,
        Rgba8::WHITE,
    )
    .unwrap_or_else(|error| panic!("point placement validates: {error}"));
    engine
        .set_point_chunk_placements(&[point])
        .unwrap_or_else(|error| panic!("point placement must fit: {error}"));
    let target = ChunkDomainRef::new(
        relation_ticket.key.dataset,
        relation_ticket.key.chunk,
        ChunkDomainKind::Relation,
    );
    let attribute =
        AttributeChunkPayload::new(target, AttributeValues::Scalar(Arc::from([1.0_f32, 2.0])))
            .unwrap_or_else(|error| panic!("scalar column validates: {error}"));
    let (request, data) = fixture(
        relation_ticket.key.dataset,
        15,
        PayloadKind::Attribute,
        ChunkPayload::Attribute(attribute),
    );
    let attribute_ticket = engine
        .request_chunk_into(request, &mut output)
        .unwrap_or_else(|error| panic!("attribute request must fit: {error}"));
    engine
        .deliver_chunk_into(attribute_ticket, data, &mut output)
        .unwrap_or_else(|error| panic!("attribute delivery must fit: {error}"));
    let start = scalar_attribute(&mut engine, &mut output, target, 16, [2.0, 4.0]);
    let end = scalar_attribute(&mut engine, &mut output, target, 17, [6.0, 8.0]);

    let descriptor = animated_relation_visual(attribute_ticket);
    let relation = RelationChunkPlacement::new(
        ChunkPlacementId::new(93),
        relation_ticket,
        RelationStyle::default(),
    )
    .and_then(|placement| placement.with_visual(descriptor))
    .unwrap_or_else(|error| panic!("visual relation placement validates: {error}"));
    engine
        .set_relation_chunk_placements(std::slice::from_ref(&relation))
        .unwrap_or_else(|error| panic!("relation placement must fit: {error}"));
    assert_eq!(
        engine.relation_chunk_placement_status(relation.id()),
        ChunkPlacementStatus::NotResident
    );

    upload(&mut engine, attribute_ticket, &mut output);
    complete(&mut engine, &mut output);
    engine
        .set_attribute_chunk_windows(&[AttributeChunkWindow::new(
            attribute_ticket,
            start,
            end,
            0.25,
        )
        .unwrap_or_else(|error| panic!("attribute window validates: {error}"))])
        .unwrap_or_else(|error| panic!("attribute window fits: {error}"));
    assert_eq!(
        engine.relation_chunk_placement_status(relation.id()),
        ChunkPlacementStatus::Resident
    );
    let mut scene = molgfx_core::Scene::new();
    engine
        .render(&scene, &super::tests::camera())
        .unwrap_or_else(|error| panic!("paged visual relation must render: {error}"));
    assert_materialized_visual(&engine);

    let buffers = buffer_count(&engine);
    let writes = relation_write_count(&engine);
    engine
        .render(&scene, &super::tests::camera())
        .unwrap_or_else(|error| panic!("stable visual relation must render: {error}"));
    assert_eq!(buffer_count(&engine), buffers);
    assert_eq!(relation_write_count(&engine), writes);

    let geometry_writes = relation_geometry_write_count(&engine);
    let visual_writes = relation_write_count(&engine);
    scene
        .set_presentation_time(0.5)
        .unwrap_or_else(|error| panic!("presentation time validates: {error}"));
    engine
        .render(&scene, &super::tests::camera())
        .unwrap_or_else(|error| panic!("animated visual relation must render: {error}"));
    assert_eq!(relation_geometry_write_count(&engine), geometry_writes);
    assert_eq!(relation_write_count(&engine), visual_writes + 1);
    assert_eq!(buffer_count(&engine), buffers);

    assert_attribute_advance(&mut engine, &scene, attribute_ticket, start, end, buffers);
}

fn assert_materialized_visual(engine: &super::Engine<crate::testing::MockDevice>) {
    let labels = buffer_labels(engine);
    assert!(labels.contains(&"paged visual instructions"));
    assert!(labels.contains(&"visual entity results"));
    assert!(labels.contains(&"paged attribute timeline materialization configuration"));
    assert_eq!(engine.derived_cache_usage().gpu_bytes, 8);
}

fn assert_attribute_advance(
    engine: &mut super::Engine<crate::testing::MockDevice>,
    scene: &molgfx_core::Scene,
    attribute: molgfx_core::ResidencyTicket,
    start: molgfx_core::ResidencyTicket,
    end: molgfx_core::ResidencyTicket,
    buffers: usize,
) {
    let geometry_writes = relation_geometry_write_count(engine);
    let visual_writes = relation_write_count(engine);
    let timeline_writes = paged_attribute_timeline_writes(engine);
    let window = AttributeChunkWindow::new(attribute, start, end, 0.75)
        .unwrap_or_else(|error| panic!("advanced attribute window validates: {error}"));
    engine
        .set_attribute_chunk_windows(&[window])
        .unwrap_or_else(|error| panic!("advanced attribute window fits: {error}"));
    engine
        .render(scene, &super::tests::camera())
        .unwrap_or_else(|error| panic!("advanced attribute timeline renders: {error}"));
    assert_eq!(relation_geometry_write_count(engine), geometry_writes);
    assert_eq!(relation_write_count(engine), visual_writes);
    assert_eq!(paged_attribute_timeline_writes(engine), timeline_writes + 1);
    assert_eq!(buffer_count(engine), buffers);
}

fn animated_relation_visual(
    attribute_ticket: molgfx_core::ResidencyTicket,
) -> Arc<ChunkVisualDescriptor> {
    let key = VisualColumnKey(71);
    let second_key = VisualColumnKey(72);
    let mut builder = VisualProgramBuilder::new();
    let width = builder
        .scalar_column(key)
        .unwrap_or_else(|error| panic!("scalar expression builds: {error}"));
    let second_width = builder
        .scalar_column(second_key)
        .unwrap_or_else(|error| panic!("second scalar expression builds: {error}"));
    let width = builder
        .add(width, second_width)
        .unwrap_or_else(|error| panic!("scalar expressions combine: {error}"));
    let time = builder
        .time()
        .unwrap_or_else(|error| panic!("time expression builds: {error}"));
    let width = builder
        .multiply(width, time)
        .unwrap_or_else(|error| panic!("animated width builds: {error}"));
    builder
        .set_width_scale(width)
        .unwrap_or_else(|error| panic!("relation width output builds: {error}"));
    Arc::new(
        ChunkVisualDescriptor::new(
            VisualStyle::new(
                builder
                    .finish()
                    .unwrap_or_else(|error| panic!("visual program builds: {error}")),
            ),
            Arc::from([
                ChunkVisualBinding::new(key, attribute_ticket),
                ChunkVisualBinding::new(second_key, attribute_ticket),
            ]),
        )
        .unwrap_or_else(|error| panic!("chunk visual validates: {error}")),
    )
}

fn scalar_attribute(
    engine: &mut super::Engine<crate::testing::MockDevice>,
    output: &mut ResidencyOutput,
    target: ChunkDomainRef,
    chunk: u64,
    values: [f32; 2],
) -> molgfx_core::ResidencyTicket {
    let payload = AttributeChunkPayload::new(target, AttributeValues::Scalar(Arc::from(values)))
        .unwrap_or_else(|error| panic!("timeline scalar column validates: {error}"));
    let (request, data) = fixture(
        target.dataset(),
        chunk,
        PayloadKind::Attribute,
        ChunkPayload::Attribute(payload),
    );
    request_deliver_upload(engine, output, request, data)
}

fn buffer_labels(engine: &super::Engine<crate::testing::MockDevice>) -> Vec<&'static str> {
    engine.device.log.buffers.lock().map_or_else(
        |_| Vec::new(),
        |buffers| buffers.iter().map(|(_, label, _)| *label).collect(),
    )
}

fn paged_attribute_timeline_writes(engine: &super::Engine<crate::testing::MockDevice>) -> usize {
    let id = engine.device.log.buffers.lock().ok().and_then(|buffers| {
        buffers
            .iter()
            .find(|(_, label, _)| {
                *label == "paged attribute timeline materialization configuration"
            })
            .map(|(id, _, _)| *id)
    });
    engine.device.log.writes.lock().map_or(0, |writes| {
        writes
            .iter()
            .filter(|(buffer, _, _, _)| Some(*buffer) == id)
            .count()
    })
}
