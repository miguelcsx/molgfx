use super::chunk_residency_tests::{engine, engine_with_derived_budget};
use super::generic_chunk_residency_tests::{
    buffer_count, fixture, relation_geometry_write_count, request_deliver_upload,
};
use super::{
    ChunkPlacementId, InstanceChunkPlacement, InstanceChunkWindow, RelationChunkPlacement,
};
use molgfx_core::{
    AnalyticSphere, AnalyticTemplate, ChunkEntityRef, ChunkPayload, ChunkSpatialKind,
    InstanceChunkPayload, LogicalRow, PagedRelation, PagedSpatialAnchor, PayloadKind,
    RelationChunkPayload, RelationStyle, ResidencyOutput, RigidInstance, SourceNamespace,
    SourceRows,
};
use molgfx_math::{Quat, Rgba8, Vec3};
use std::sync::Arc;

#[test]
fn paged_instance_timeline_samples_resident_pages_without_rebuilding_batches() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let dataset = molgfx_core::DatasetId::new(0x1_0000_0017);
    let base = instance_chunk(&mut engine, &mut output, dataset, 21, 0.0);
    let start = instance_chunk(&mut engine, &mut output, dataset, 22, 10.0);
    let end = instance_chunk(&mut engine, &mut output, dataset, 23, 20.0);
    let id = ChunkPlacementId::new(301);
    engine
        .set_instance_chunk_placements(&[InstanceChunkPlacement::new(
            id,
            base,
            template(),
            Rgba8::WHITE,
        )])
        .unwrap_or_else(|error| panic!("instance placement validates: {error}"));
    engine
        .set_instance_chunk_windows(&[InstanceChunkWindow::new(id, start, end, 0.25)
            .unwrap_or_else(|error| panic!("instance window validates: {error}"))])
        .unwrap_or_else(|error| panic!("instance window fits: {error}"));
    install_anchored_relations(&mut engine, &mut output, dataset, id, base);

    let scene = molgfx_core::Scene::new();
    engine
        .render(&scene, &super::tests::camera())
        .unwrap_or_else(|error| panic!("paged instance timeline renders: {error}"));
    let buffers = buffer_count(&engine);
    let writes = instance_config_writes(&engine);
    let materialization_writes = instance_timeline_writes(&engine);
    let relation_geometry_writes = relation_geometry_write_count(&engine);
    let timeline_writes = relation_timeline_writes(&engine);
    let start_offset = instance_offset(&engine, start);
    let end_offset = instance_offset(&engine, end);
    assert_eq!(
        latest_timeline_config(&engine),
        [start_offset, end_offset, 0.25_f32.to_bits()]
    );
    assert_eq!(engine.derived_cache_usage().gpu_bytes, 2 * 32);
    assert!(relations_read_materialized_instances(&engine));

    engine
        .set_instance_chunk_windows(&[InstanceChunkWindow::new(id, start, end, 0.75)
            .unwrap_or_else(|error| panic!("advanced window validates: {error}"))])
        .unwrap_or_else(|error| panic!("advanced window fits: {error}"));
    if let Ok(mut passes) = engine.device.log.compute_passes.lock() {
        passes.clear();
    }
    engine
        .render(&scene, &super::tests::camera())
        .unwrap_or_else(|error| panic!("advanced timeline renders: {error}"));
    assert_eq!(buffer_count(&engine), buffers);
    assert_eq!(instance_config_writes(&engine), writes);
    assert_eq!(
        instance_timeline_writes(&engine),
        materialization_writes + 1
    );
    assert_eq!(
        relation_geometry_write_count(&engine),
        relation_geometry_writes
    );
    assert_eq!(relation_timeline_writes(&engine), timeline_writes + 1);
    assert!(compute_pass_recorded(
        &engine,
        "dynamic relation resolution"
    ));
    assert_eq!(
        latest_timeline_config(&engine),
        [start_offset, end_offset, 0.75_f32.to_bits()]
    );
}

#[test]
fn zero_budget_paged_instances_sample_two_resident_pages_directly() {
    let mut engine = engine_with_derived_budget(super::DerivedCacheBudget {
        cpu_bytes: 0,
        gpu_bytes: 0,
    });
    let mut output = ResidencyOutput::default();
    let dataset = molgfx_core::DatasetId::new(0x1_0000_0018);
    let base = instance_chunk(&mut engine, &mut output, dataset, 31, 0.0);
    let start = instance_chunk(&mut engine, &mut output, dataset, 32, 10.0);
    let end = instance_chunk(&mut engine, &mut output, dataset, 33, 20.0);
    let id = ChunkPlacementId::new(311);
    engine
        .set_instance_chunk_placements(&[InstanceChunkPlacement::new(
            id,
            base,
            template(),
            Rgba8::WHITE,
        )])
        .unwrap_or_else(|error| panic!("direct placement validates: {error}"));
    let window = InstanceChunkWindow::new(id, start, end, 0.5)
        .unwrap_or_else(|error| panic!("direct window validates: {error}"));
    engine
        .set_instance_chunk_windows(&[window])
        .unwrap_or_else(|error| panic!("direct window fits: {error}"));
    engine
        .render(&molgfx_core::Scene::new(), &super::tests::camera())
        .unwrap_or_else(|error| panic!("direct timeline renders: {error}"));

    assert_eq!(engine.derived_cache_usage().gpu_bytes, 0);
    assert_eq!(
        buffer_id(&engine, "materialized paged instance timeline"),
        u32::MAX
    );
    assert_eq!(
        latest_direct_config(&engine),
        [
            instance_offset(&engine, start),
            instance_offset(&engine, end),
            0.5_f32.to_bits(),
        ]
    );
}

fn compute_pass_recorded(
    engine: &super::Engine<crate::testing::MockDevice>,
    label: &'static str,
) -> bool {
    engine
        .device
        .log
        .compute_passes
        .lock()
        .is_ok_and(|passes| passes.contains(&label))
}

fn install_anchored_relations(
    engine: &mut super::Engine<crate::testing::MockDevice>,
    output: &mut ResidencyOutput,
    dataset: molgfx_core::DatasetId,
    instance: ChunkPlacementId,
    source: molgfx_core::ResidencyTicket,
) {
    let instance = ChunkEntityRef::new(
        dataset,
        source.key.chunk,
        instance,
        LogicalRow::new(100),
        ChunkSpatialKind::Instance,
    );
    let payload = RelationChunkPayload::new(Arc::from([
        PagedRelation {
            start: PagedSpatialAnchor::World(Vec3::ZERO),
            end: PagedSpatialAnchor::Entity(instance),
        },
        PagedRelation {
            start: PagedSpatialAnchor::World(Vec3::Y),
            end: PagedSpatialAnchor::Entity(instance),
        },
    ]))
    .unwrap_or_else(|error| panic!("relation payload validates: {error}"));
    let (request, data) = fixture(
        dataset,
        24,
        PayloadKind::RelationBatch,
        ChunkPayload::RelationBatch(payload),
    );
    let ticket = request_deliver_upload(engine, output, request, data);
    let placement =
        RelationChunkPlacement::new(ChunkPlacementId::new(302), ticket, RelationStyle::default())
            .unwrap_or_else(|error| panic!("relation placement validates: {error}"));
    engine
        .set_relation_chunk_placements(&[placement])
        .unwrap_or_else(|error| panic!("relation placement fits: {error}"));
}

fn instance_chunk(
    engine: &mut super::Engine<crate::testing::MockDevice>,
    output: &mut ResidencyOutput,
    dataset: molgfx_core::DatasetId,
    chunk: u64,
    x: f32,
) -> molgfx_core::ResidencyTicket {
    let payload = InstanceChunkPayload::new(Arc::from([
        rigid(Vec3::new(x, 0.0, 0.0)),
        rigid(Vec3::new(x + 1.0, 0.0, 0.0)),
    ]))
    .unwrap_or_else(|error| panic!("instance payload validates: {error}"));
    let (request, data) = fixture(
        dataset,
        chunk,
        PayloadKind::InstanceBatch,
        ChunkPayload::InstanceBatch(payload),
    );
    request_deliver_upload(engine, output, request, data)
}

fn template() -> Arc<AnalyticTemplate> {
    Arc::new(
        AnalyticTemplate::new(
            Arc::from([AnalyticSphere {
                center: [0.0; 3],
                radius: 0.5,
            }]),
            Arc::from([]),
            SourceRows::ordered(SourceNamespace(301), 1),
        )
        .unwrap_or_else(|error| panic!("analytic template validates: {error}")),
    )
}

fn rigid(translation: Vec3) -> RigidInstance {
    RigidInstance::new(translation, Quat::IDENTITY, 1.0)
        .unwrap_or_else(|error| panic!("rigid instance validates: {error}"))
}

fn instance_config_writes(engine: &super::Engine<crate::testing::MockDevice>) -> usize {
    let id = buffer_id(engine, "paged instance configuration");
    engine.device.log.writes.lock().map_or(0, |writes| {
        writes
            .iter()
            .filter(|(buffer, _, _, _)| *buffer == id)
            .count()
    })
}

fn relation_timeline_writes(engine: &super::Engine<crate::testing::MockDevice>) -> usize {
    let id = buffer_id(engine, "paged relation rigid timeline");
    engine.device.log.writes.lock().map_or(0, |writes| {
        writes
            .iter()
            .filter(|(buffer, _, _, _)| *buffer == id)
            .count()
    })
}

fn instance_timeline_writes(engine: &super::Engine<crate::testing::MockDevice>) -> usize {
    let id = buffer_id(engine, "paged instance timeline configuration");
    engine.device.log.writes.lock().map_or(0, |writes| {
        writes
            .iter()
            .filter(|(buffer, _, _, _)| *buffer == id)
            .count()
    })
}

fn latest_timeline_config(engine: &super::Engine<crate::testing::MockDevice>) -> [u32; 3] {
    latest_config_words(engine, "paged instance timeline configuration", [20, 24, 0])
}

fn latest_direct_config(engine: &super::Engine<crate::testing::MockDevice>) -> [u32; 3] {
    latest_config_words(engine, "paged instance configuration", [28, 44, 36])
}

fn latest_config_words(
    engine: &super::Engine<crate::testing::MockDevice>,
    label: &'static str,
    offsets: [usize; 3],
) -> [u32; 3] {
    let id = buffer_id(engine, label);
    let index = engine.device.log.writes.lock().ok().and_then(|writes| {
        writes
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, (buffer, _, _, _))| (*buffer == id).then_some(index))
    });
    let payload = index.and_then(|index| {
        engine
            .device
            .log
            .write_payloads
            .lock()
            .ok()
            .and_then(|payloads| payloads.get(index).cloned())
    });
    let Some(payload) = payload else {
        panic!("instance configuration write must be recorded")
    };
    let word = |offset: usize| {
        u32::from_ne_bytes(
            payload[offset..offset + 4]
                .try_into()
                .unwrap_or_else(|_| panic!("configuration word must exist")),
        )
    };
    offsets.map(word)
}

fn instance_offset(
    engine: &super::Engine<crate::testing::MockDevice>,
    ticket: molgfx_core::ResidencyTicket,
) -> u32 {
    let resident = engine
        .resident_generic_chunk(ticket)
        .unwrap_or_else(|| panic!("instance page must be resident"));
    u32::try_from(resident.byte_offset / 32)
        .unwrap_or_else(|_| panic!("fixture offset must fit u32"))
}

fn relations_read_materialized_instances(
    engine: &super::Engine<crate::testing::MockDevice>,
) -> bool {
    let materialized = buffer_id(engine, "materialized paged instance timeline");
    engine
        .device
        .log
        .buffer_bindings
        .lock()
        .is_ok_and(|bindings| {
            bindings.iter().any(|(group, binding, buffer, _, _)| {
                *group == "group1: paged relation resolver"
                    && [1, 2, 6, 7].contains(binding)
                    && *buffer == materialized
            })
        })
}

fn buffer_id(engine: &super::Engine<crate::testing::MockDevice>, label: &'static str) -> u32 {
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
