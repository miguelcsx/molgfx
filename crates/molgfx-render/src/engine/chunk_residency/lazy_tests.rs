use crate::engine::chunk_residency_tests::{
    complete, engine, frame_fixture, request_and_deliver, upload,
};
use crate::engine::generic_chunk_residency_tests::{generic_fixtures, request_deliver_upload};
use crate::engine::{
    ChunkPlacementId, Engine, InstanceChunkPlacement, PointChunkPlacement, RelationChunkPlacement,
};
use crate::testing::MockDevice;
use molgfx_core::{AnalyticSphere, AnalyticTemplate, ResidencyOutput, SourceNamespace, SourceRows};
use molgfx_math::{Mat4, Rgba8};
use std::sync::Arc;

#[test]
fn unused_paging_reserves_only_four_small_placeholder_buffers() {
    // Four placeholders, not five: the display-coordinate backing is only
    // allocated when a trajectory window exists, so an idle scene never pays
    // for a second coordinate buffer.
    let engine = engine();
    let buffers = engine.device.log.buffers.lock().expect("buffer log");
    let placeholders: Vec<_> = buffers
        .iter()
        .filter(|(_, label, _)| label.starts_with("unused resident "))
        .collect();
    assert_eq!(placeholders.len(), 4);
    assert_eq!(
        placeholders.iter().map(|(_, _, bytes)| *bytes).sum::<u64>(),
        1_024
    );
}

#[test]
fn frame_payload_allocates_only_the_trajectory_backing() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let (ticket, _, _) =
        request_and_deliver(&mut engine, frame_fixture(19, 20, 1, 2.0), &mut output);
    upload(&mut engine, ticket, &mut output);
    assert_eq!(
        engine.chunk_residency.buffer.label,
        "unused resident structure chunks"
    );
    // A frame payload is what makes the interpolation target necessary, so
    // both trajectory backings appear together.
    assert_eq!(
        engine
            .chunk_residency
            .display_buffer
            .as_ref()
            .map(|buffer| buffer.label),
        Some("resident display coordinates")
    );
    assert_eq!(
        engine.chunk_residency.frame_buffer.label,
        "resident trajectory frames"
    );
    assert_eq!(
        engine.chunk_residency.cluster_buffer.label,
        "unused resident structure chunk clusters"
    );
    assert_eq!(
        engine.chunk_residency.bonds.buffer.label,
        "unused resident provider bonds"
    );
}

#[test]
fn generic_points_upload_exact_coordinates_into_the_drawable_backing() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let (request, data) = generic_fixtures().remove(0);
    request_deliver_upload(&mut engine, &mut output, request, data);
    // Paged chunks read the canonical backing unless a trajectory window
    // exists, so the payload lands in that buffer.
    let display = engine.chunk_residency.buffer.id;
    let writes = engine.device.log.writes.lock().expect("write log");
    let index = writes
        .iter()
        .position(|(id, _, _, _)| *id == display)
        .expect("display payload write");
    let payloads = engine
        .device
        .log
        .write_payloads
        .lock()
        .expect("payload log");
    let positions = [[1.0_f32, 2.0, 3.0], [4.0, 5.0, 6.0]];
    assert_eq!(payloads[index], bytemuck::cast_slice::<_, u8>(&positions));
    assert_eq!(
        engine.chunk_residency.frame_buffer.label,
        "unused resident trajectory frames"
    );
    assert_eq!(
        engine.chunk_residency.bonds.buffer.label,
        "unused resident provider bonds"
    );
}

#[test]
fn backing_reset_rebinds_every_paged_consumer_without_an_intermediate_frame() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let tickets: Vec<_> = generic_fixtures()
        .into_iter()
        .map(|(request, data)| request_deliver_upload(&mut engine, &mut output, request, data))
        .collect();
    engine
        .set_point_chunk_placements(&[PointChunkPlacement::new(
            ChunkPlacementId::new(91),
            tickets[0],
            Mat4::IDENTITY,
            3.0,
            Rgba8::WHITE,
        )
        .expect("point placement")])
        .expect("point placements");
    let template = Arc::new(
        AnalyticTemplate::new(
            Arc::from([AnalyticSphere {
                center: [0.0; 3],
                radius: 1.0,
            }]),
            Arc::from([]),
            SourceRows::ordered(SourceNamespace(300), 1),
        )
        .expect("template"),
    );
    engine
        .set_instance_chunk_placements(&[InstanceChunkPlacement::new(
            ChunkPlacementId::new(92),
            tickets[1],
            template,
            Rgba8::WHITE,
        )])
        .expect("instance placements");
    engine
        .set_relation_chunk_placements(&[RelationChunkPlacement::new(
            ChunkPlacementId::new(93),
            tickets[3],
            molgfx_core::RelationStyle::default(),
        )
        .expect("relation placement")])
        .expect("relation placements");
    let scene = molgfx_core::Scene::new();
    let camera = crate::engine::tests::camera();
    engine.render(&scene, &camera).expect("first frame");
    let plan = crate::engine::chunk_draw_plan::ResidentAttributeMaterialization {
        ticket: tickets[2],
        start_byte_offset: 0,
        end_byte_offset: 8,
        output_byte_offset: 16,
        word_count: 2,
        interpolation: 0.5,
    };
    sync_attributes(&mut engine, plan);
    assert_current_bindings(&engine);
    let old_source = engine.chunk_residency.buffer.id;
    engine.chunk_device_lost_into(&mut output).expect("reset");
    for ticket in tickets {
        upload(&mut engine, ticket, &mut output);
    }
    complete(&mut engine, &mut output);
    sync_attributes(&mut engine, plan);
    let attribute_source = engine
        .device
        .log
        .buffer_bindings
        .lock()
        .expect("bindings")
        .iter()
        .rev()
        .find(|(label, binding, _, _, _)| {
            *label == "paged attribute timeline materialization" && *binding == 0
        })
        .map(|(_, _, buffer, _, _)| *buffer);
    assert_eq!(attribute_source, Some(engine.chunk_residency.buffer.id));
    engine.render(&scene, &camera).expect("restored frame");
    sync_attributes(&mut engine, plan);
    assert_ne!(engine.chunk_residency.buffer.id, old_source);
    assert_current_bindings(&engine);
}

fn sync_attributes(
    engine: &mut Engine<MockDevice>,
    plan: crate::engine::chunk_draw_plan::ResidentAttributeMaterialization,
) {
    engine
        .scene_gpu
        .sync_paged_attribute_timelines(
            &engine.device,
            &engine.queue,
            &engine.chunk_residency.buffer,
            &[plan],
            engine.chunk_residency.binding_revision,
        )
        .expect("attribute bindings");
}

fn assert_current_bindings(engine: &Engine<MockDevice>) {
    let source = engine.chunk_residency.buffer.id;
    // Paged chunks read whichever coordinate backing is current: the
    // interpolation target while a trajectory window exists, the canonical
    // arena otherwise.
    let display = match engine.chunk_residency.display_buffer.as_ref() {
        Some(buffer) => buffer.id,
        None => source,
    };
    let bindings = engine.device.log.buffer_bindings.lock().expect("bindings");
    for (label, slot, expected) in [
        ("paged structure chunks", 0, display),
        ("group1: generic instance culling", 0, source),
        ("group2: generic analytic instances", 7, source),
        ("paged attribute timeline materialization", 0, source),
    ] {
        let bound = bindings
            .iter()
            .rev()
            .find(|(group, binding, _, _, _)| *group == label && *binding == slot)
            .expect("bound source");
        assert_eq!(
            bound.2, expected,
            "{label} must use the current physical backing"
        );
    }
    let latest_relation: Vec<_> = bindings
        .iter()
        .rev()
        .filter(|(group, _, _, _, _)| *group == "group1: paged relation resolver")
        .take(10)
        .collect();
    assert!(
        latest_relation
            .iter()
            .any(|(_, slot, id, _, _)| [1, 2].contains(slot) && *id == source),
        "point relation resolver must use the current generic backing"
    );
}
