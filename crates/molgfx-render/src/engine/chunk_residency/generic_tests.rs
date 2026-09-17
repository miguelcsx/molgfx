use super::chunk_residency_tests::{
    complete, engine, fixture as structure_fixture, frame_fixture, upload,
};
use super::{
    ChunkPlacementId, ChunkPlacementStatus, ChunkRepresentation, InstanceChunkPlacement,
    PickEntity, PointChunkPlacement, RelationChunkPlacement, StructureChunkPlacement,
    TrajectoryChunkWindow,
};
use molgfx_core::{
    AnalyticCapsule, AnalyticSphere, AnalyticTemplate, AttributeChunkPayload, AttributeValues,
    ChunkBounds, ChunkData, ChunkDescriptor, ChunkDomainKind, ChunkDomainRef, ChunkEntityRef,
    ChunkFootprint, ChunkId, ChunkPayload, ChunkSpan, ChunkSpatialKind, DatasetCatalog, DatasetId,
    InstanceChunkPayload, LogicalRow, PagedRelation, PagedSpatialAnchor, PayloadKind,
    PointChunkPayload, RelationChunkPayload, RelationStyle, ResidencyClass, ResidencyDetail,
    ResidencyKey, ResidencyOutput, ResidencyRequest, RigidInstance, SourceNamespace, SourceRows,
    TemplatePartChunkRef,
};
use molgfx_math::{Mat4, Quat, Rgba8, Vec3};
use std::sync::Arc;

#[test]
fn all_generic_payloads_share_one_bounded_arena_and_preserve_native_strides() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let fixtures = generic_fixtures();
    let expected = [
        (PayloadKind::PointBatch, 12),
        (PayloadKind::InstanceBatch, 32),
        (PayloadKind::Attribute, 4),
        (PayloadKind::RelationBatch, 0),
    ];
    let mut tickets = Vec::new();
    for (request, data) in fixtures {
        let ticket = engine
            .request_chunk_into(request, &mut output)
            .unwrap_or_else(|error| panic!("generic request must fit: {error}"));
        engine
            .deliver_chunk_into(ticket, data, &mut output)
            .unwrap_or_else(|error| panic!("generic delivery must fit: {error}"));
        upload(&mut engine, ticket, &mut output);
        tickets.push(ticket);
    }
    complete(&mut engine, &mut output);

    for (ticket, (kind, stride)) in tickets.iter().copied().zip(expected) {
        let resident = engine
            .resident_generic_chunk(ticket)
            .unwrap_or_else(|| panic!("{kind:?} must become resident"));
        assert_eq!(resident.kind, kind);
        assert_eq!(resident.stride, stride);
        assert_eq!(resident.local_rows, 2);
        if kind == PayloadKind::RelationBatch {
            assert_eq!(resident.byte_len, 104);
        }
        if kind == PayloadKind::Attribute {
            assert_eq!(
                resident.target,
                Some(ChunkDomainRef::new(
                    DatasetId::new(0x1_0000_0007),
                    ChunkId::new(11),
                    ChunkDomainKind::Point,
                ))
            );
        }
    }
    let metrics = engine.chunk_residency_metrics();
    assert_eq!(metrics.tracked_chunks, 4);
    assert_eq!(metrics.arena.resident_bytes, 4 * 256);

    let report = engine
        .chunk_device_lost_into(&mut output)
        .unwrap_or_else(|error| panic!("device loss must release generic pages: {error}"));
    assert_eq!(report.invalidated, 4);
    assert_eq!(engine.chunk_residency_metrics().arena.resident_bytes, 0);
    assert!(
        tickets
            .into_iter()
            .all(|ticket| engine.resident_generic_chunk(ticket).is_none())
    );
}

#[test]
fn resident_generic_points_join_the_indirect_batch_and_pick_global_rows() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let Some((request, data)) = generic_fixtures().into_iter().next() else {
        panic!("generic point fixture must exist")
    };
    let ticket = engine
        .request_chunk_into(request, &mut output)
        .unwrap_or_else(|error| panic!("point request must fit: {error}"));
    engine
        .deliver_chunk_into(ticket, data, &mut output)
        .unwrap_or_else(|error| panic!("point delivery must fit: {error}"));
    upload(&mut engine, ticket, &mut output);
    complete(&mut engine, &mut output);

    let placement = PointChunkPlacement::new(
        ChunkPlacementId::new(91),
        ticket,
        Mat4::IDENTITY,
        3.0,
        Rgba8::opaque(40, 180, 220),
    )
    .unwrap_or_else(|error| panic!("point placement must validate: {error}"));
    engine
        .set_point_chunk_placements(&[placement])
        .unwrap_or_else(|error| panic!("point placement must fit: {error}"));
    let scene = molgfx_core::Scene::new();
    engine
        .render(&scene, &super::tests::camera())
        .unwrap_or_else(|error| panic!("generic points must render: {error}"));

    assert_eq!(
        engine.point_chunk_placement_status(placement.id()),
        ChunkPlacementStatus::Resident
    );
    let args = buffer_id(&engine, "paged indirect command arena");
    assert_eq!(indirect_count(&engine, args), 1);
    let page = engine.scene_gpu.test_chunk_pick_page(
        ticket.key.dataset,
        ticket.key.chunk,
        molgfx_core::EntityKind::Point,
    );
    let Some(page) = page else {
        panic!("generic points must own a point picking page")
    };
    if let Ok(mut value) = engine.device.log.pick_resident_page.lock() {
        *value = page;
    }
    if let Ok(mut value) = engine.device.log.pick_local_row.lock() {
        *value = 1;
    }
    let picked = engine
        .pick(0, 0)
        .unwrap_or_else(|error| panic!("generic point pick must resolve: {error}"));
    let Some(picked) = picked else {
        panic!("mock point must resolve")
    };
    let PickEntity::Structure(identity) = picked.entity else {
        panic!("global point identity expected")
    };
    assert_eq!(identity.kind(), molgfx_core::EntityKind::Point);
    assert_eq!(identity.row(), LogicalRow::new(101));

    let buffers = buffer_count(&engine);
    let writes = paged_write_count(&engine);
    engine
        .render(&scene, &super::tests::camera())
        .unwrap_or_else(|error| panic!("stable generic point frame must render: {error}"));
    assert_eq!(buffer_count(&engine), buffers);
    assert_eq!(paged_write_count(&engine), writes);
    assert_eq!(indirect_count(&engine, args), 2);
}

#[test]
fn resident_instance_chunks_reuse_analytic_culling_and_flat_part_picking() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let Some((request, data)) = generic_fixtures().into_iter().nth(1) else {
        panic!("generic instance fixture must exist")
    };
    let ticket = engine
        .request_chunk_into(request, &mut output)
        .unwrap_or_else(|error| panic!("instance request must fit: {error}"));
    engine
        .deliver_chunk_into(ticket, data, &mut output)
        .unwrap_or_else(|error| panic!("instance delivery must fit: {error}"));
    upload(&mut engine, ticket, &mut output);
    complete(&mut engine, &mut output);

    let template = Arc::new(
        AnalyticTemplate::new(
            Arc::from([AnalyticSphere {
                center: [0.0; 3],
                radius: 0.5,
            }]),
            Arc::from([AnalyticCapsule::new(Vec3::ZERO, Vec3::Z, 0.2)
                .unwrap_or_else(|error| panic!("capsule validates: {error}"))]),
            SourceRows::keyed(SourceNamespace(501), Arc::from([31_u64, 47]))
                .unwrap_or_else(|error| panic!("template rows validate: {error}")),
        )
        .unwrap_or_else(|error| panic!("template validates: {error}")),
    );
    let placement = InstanceChunkPlacement::new(
        ChunkPlacementId::new(92),
        ticket,
        template,
        Rgba8::opaque(90, 160, 220),
    );
    engine
        .set_instance_chunk_placements(std::slice::from_ref(&placement))
        .unwrap_or_else(|error| panic!("instance placement must fit: {error}"));
    let scene = molgfx_core::Scene::new();
    engine
        .render(&scene, &super::tests::camera())
        .unwrap_or_else(|error| panic!("paged instances must render: {error}"));

    assert_eq!(
        engine.instance_chunk_placement_status(placement.id()),
        ChunkPlacementStatus::Resident
    );
    let draws = engine
        .device
        .log
        .indirect_draws
        .lock()
        .map_or(0, |value| value.len());
    assert_eq!(draws, 2, "sphere and capsule use homogeneous draws");
    let page = engine.scene_gpu.test_chunk_pick_page(
        ticket.key.dataset,
        ticket.key.chunk,
        molgfx_core::EntityKind::TemplatePart,
    );
    let Some(page) = page else {
        panic!("instance occurrences must own a template-part page")
    };
    if let Ok(mut value) = engine.device.log.pick_resident_page.lock() {
        *value = page;
    }
    if let Ok(mut value) = engine.device.log.pick_local_row.lock() {
        *value = 3;
    }
    let picked = engine
        .pick(0, 0)
        .unwrap_or_else(|error| panic!("template-part pick must resolve: {error}"));
    let Some(picked) = picked else {
        panic!("mock template part must resolve")
    };
    let PickEntity::Structure(identity) = picked.entity else {
        panic!("global template-part identity expected")
    };
    assert_eq!(identity.kind(), molgfx_core::EntityKind::TemplatePart);
    assert_eq!(identity.row(), LogicalRow::new(3));

    let buffers = buffer_count(&engine);
    let writes = paged_write_count(&engine);
    engine
        .render(&scene, &super::tests::camera())
        .unwrap_or_else(|error| panic!("stable paged instance frame must render: {error}"));
    assert_eq!(buffer_count(&engine), buffers);
    assert_eq!(paged_write_count(&engine), writes);
    let template_uploads = engine.device.log.buffers.lock().map_or(0, |values| {
        values
            .iter()
            .filter(|(_, label, _)| *label == "generic analytic template spheres")
            .count()
    });
    assert_eq!(template_uploads, 1);
}

#[test]
#[allow(clippy::too_many_lines)]
fn paged_anchors_resolve_exact_occurrences_to_local_gpu_rows() {
    use super::chunk_draw_plan::ResidentSpatialAnchor;

    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let Some((request, data)) = generic_fixtures().into_iter().next() else {
        panic!("generic point fixture must exist")
    };
    let ticket = engine
        .request_chunk_into(request, &mut output)
        .unwrap_or_else(|error| panic!("point request must fit: {error}"));
    engine
        .deliver_chunk_into(ticket, data, &mut output)
        .unwrap_or_else(|error| panic!("point delivery must fit: {error}"));
    upload(&mut engine, ticket, &mut output);
    complete(&mut engine, &mut output);
    let model = Mat4::from_translation(Vec3::new(8.0, 9.0, 10.0));
    let placement =
        PointChunkPlacement::new(ChunkPlacementId::new(91), ticket, model, 2.0, Rgba8::WHITE)
            .unwrap_or_else(|error| panic!("point placement must validate: {error}"));
    engine
        .set_point_chunk_placements(&[placement])
        .unwrap_or_else(|error| panic!("point placement must fit: {error}"));
    let reference = ChunkEntityRef::new(
        ticket.key.dataset,
        ticket.key.chunk,
        ChunkPlacementId::new(91),
        LogicalRow::new(101),
        ChunkSpatialKind::Point,
    );
    let resolved = engine
        .chunk_residency
        .resolve_spatial_anchor(PagedSpatialAnchor::Entity(reference));
    assert!(matches!(
        resolved,
        Some(ResidentSpatialAnchor::Position {
            local_row: 1,
            model_to_world,
            ..
        }) if model_to_world == model
    ));

    let stale = ChunkEntityRef::new(
        ticket.key.dataset,
        ticket.key.chunk,
        ChunkPlacementId::new(999),
        LogicalRow::new(101),
        ChunkSpatialKind::Point,
    );
    assert!(
        engine
            .chunk_residency
            .resolve_spatial_anchor(PagedSpatialAnchor::Entity(stale))
            .is_none()
    );

    let Some((request, data)) = generic_fixtures().into_iter().nth(1) else {
        panic!("generic instance fixture must exist")
    };
    let instance_ticket = engine
        .request_chunk_into(request, &mut output)
        .unwrap_or_else(|error| panic!("instance request must fit: {error}"));
    engine
        .deliver_chunk_into(instance_ticket, data, &mut output)
        .unwrap_or_else(|error| panic!("instance delivery must fit: {error}"));
    upload(&mut engine, instance_ticket, &mut output);
    complete(&mut engine, &mut output);
    let template = Arc::new(
        AnalyticTemplate::new(
            Arc::from([AnalyticSphere {
                center: [0.0; 3],
                radius: 0.5,
            }]),
            Arc::from([AnalyticCapsule::new(Vec3::ZERO, Vec3::Z, 0.2)
                .unwrap_or_else(|error| panic!("capsule validates: {error}"))]),
            SourceRows::ordered(SourceNamespace(700), 2),
        )
        .unwrap_or_else(|error| panic!("template validates: {error}")),
    );
    engine
        .set_instance_chunk_placements(&[InstanceChunkPlacement::new(
            ChunkPlacementId::new(92),
            instance_ticket,
            template,
            Rgba8::WHITE,
        )])
        .unwrap_or_else(|error| panic!("instance placement must fit: {error}"));
    let instance = ChunkEntityRef::new(
        instance_ticket.key.dataset,
        instance_ticket.key.chunk,
        ChunkPlacementId::new(92),
        LogicalRow::new(100),
        ChunkSpatialKind::Instance,
    );
    let resolved = engine
        .chunk_residency
        .resolve_spatial_anchor(PagedSpatialAnchor::TemplatePart(TemplatePartChunkRef::new(
            instance, 1,
        )));
    assert!(matches!(
        resolved,
        Some(ResidentSpatialAnchor::Rigid {
            local_row: 0,
            local_position,
            ..
        }) if local_position == Vec3::Z * 0.5
    ));
}

#[test]
fn paged_relations_wait_for_exact_sources_then_share_one_indirect_draw() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let Some((relation_request, relation_data)) = generic_fixtures().into_iter().nth(3) else {
        panic!("generic relation fixture must exist")
    };
    let relation_ticket =
        request_deliver_upload(&mut engine, &mut output, relation_request, relation_data);
    let relation = RelationChunkPlacement::new(
        ChunkPlacementId::new(93),
        relation_ticket,
        RelationStyle::default(),
    )
    .unwrap_or_else(|error| panic!("relation placement validates: {error}"));
    engine
        .set_relation_chunk_placements(std::slice::from_ref(&relation))
        .unwrap_or_else(|error| panic!("relation placement must fit: {error}"));
    assert_eq!(
        engine.relation_chunk_placement_status(relation.id()),
        ChunkPlacementStatus::NotResident
    );

    let Some((point_request, point_data)) = generic_fixtures().into_iter().next() else {
        panic!("generic point fixture must exist")
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
    assert_eq!(
        engine.relation_chunk_placement_status(relation.id()),
        ChunkPlacementStatus::Resident
    );

    let scene = molgfx_core::Scene::new();
    engine
        .render(&scene, &super::tests::camera())
        .unwrap_or_else(|error| panic!("paged relations must render: {error}"));
    let args = buffer_id(&engine, "interaction glyph indirect arguments");
    assert_eq!(indirect_count(&engine, args), 1);
    let page = engine.scene_gpu.test_chunk_pick_page(
        relation_ticket.key.dataset,
        relation_ticket.key.chunk,
        molgfx_core::EntityKind::Relation,
    );
    let Some(page) = page else {
        panic!("resident relation chunk must own a picking page")
    };
    if let Ok(mut value) = engine.device.log.pick_resident_page.lock() {
        *value = page;
    }
    if let Ok(mut value) = engine.device.log.pick_local_row.lock() {
        *value = 1;
    }
    let picked = engine
        .pick(0, 0)
        .unwrap_or_else(|error| panic!("relation pick must resolve: {error}"));
    let Some(picked) = picked else {
        panic!("mock relation must resolve")
    };
    let PickEntity::Structure(identity) = picked.entity else {
        panic!("global relation identity expected")
    };
    assert_eq!(identity.kind(), molgfx_core::EntityKind::Relation);
    assert_eq!(identity.row(), LogicalRow::new(101));

    let buffers = buffer_count(&engine);
    let writes = relation_write_count(&engine);
    engine
        .render(&scene, &super::tests::camera())
        .unwrap_or_else(|error| panic!("stable paged relation frame must render: {error}"));
    assert_eq!(buffer_count(&engine), buffers);
    assert_eq!(relation_write_count(&engine), writes);
    assert_eq!(indirect_count(&engine, args), 2);
}

#[path = "generic_tests/support.rs"]
mod support;
pub(super) use support::{
    assert_compute_precedes, buffer_count, buffer_id, fixture, generic_fixtures, indirect_count,
    paged_write_count, relation_geometry_write_count, relation_write_count, request_deliver_upload,
};

#[path = "generic_tests/trajectory.rs"]
mod trajectory;
