use super::tests::{camera, engine, structure};
use pdviewx_core::{
    AnalyticSphere, AnalyticTemplate, AttributeColumn, AttributeValues, ChunkId, DatasetId,
    EntityKind, GpuPickToken, InstanceBatch, LogicalRow, PlaybackMode, PointBatch, PointGlyph,
    PointStyle, Relation, RelationBatch, RelationStyle, RigidInstance, RowDomain, RowEntityRef,
    Scene, SourceNamespace, SourceRows, SpatialAnchor, TemplatePartRef, TimeWarp, Timeline,
    VisualDescriptor, VisualProgramBuilder, VisualStyle,
};
use pdviewx_math::{Mat4, Quat, Vec3};
use std::sync::Arc;

#[test]
fn static_world_relations_share_one_indirect_analytic_draw() {
    let mut scene = Scene::new();
    let relation = Relation {
        start: SpatialAnchor::world(Vec3::ZERO).unwrap_or_else(|error| panic!("{error}")),
        end: SpatialAnchor::world(Vec3::X).unwrap_or_else(|error| panic!("{error}")),
    };
    let batch = RelationBatch::new(
        Arc::from([relation, relation]),
        SourceRows::ordered(SourceNamespace(710), 2),
        RelationStyle::default(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let _handle = scene
        .add_relation_batch(batch)
        .unwrap_or_else(|error| panic!("{error}"));

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let draws = engine
        .device
        .log
        .indirect_draws
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        draws.len(),
        1,
        "one homogeneous relation stream is one draw"
    );
    drop(draws);
    let dispatches = engine
        .device
        .log
        .dispatches
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        dispatches.as_slice(),
        &[(1, 1, 1), (1, 1, 1)],
        "relations reset and compact visibility entirely on the GPU"
    );
}

#[test]
fn relation_visual_timeline_runs_on_gpu_without_repacking_glyph_rows() {
    let mut scene = Scene::new();
    let relation = Relation {
        start: SpatialAnchor::world(Vec3::ZERO).unwrap_or_else(|error| panic!("{error}")),
        end: SpatialAnchor::world(Vec3::X).unwrap_or_else(|error| panic!("{error}")),
    };
    let handle = scene
        .add_relation_batch(
            RelationBatch::new(
                Arc::from([relation, relation]),
                SourceRows::ordered(SourceNamespace(719), 2),
                RelationStyle::default(),
            )
            .unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let domain = RowDomain::Relations(handle);
    let attribute = scene
        .add_attribute(
            AttributeColumn::new(
                domain,
                "relation width",
                AttributeValues::Scalar(Arc::from([1.0, 1.0])),
            )
            .unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let mut builder = VisualProgramBuilder::new();
    let width = builder
        .scalar_attribute(attribute)
        .unwrap_or_else(|error| panic!("{error}"));
    builder
        .set_width_scale(width)
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .set_domain_visual(
            domain,
            VisualDescriptor::new(VisualStyle::new(
                builder.finish().unwrap_or_else(|error| panic!("{error}")),
            )),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let warp = TimeWarp::new(0.0, 0.0, 1.0, [0.0, 1.0], PlaybackMode::Clamp)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut timeline = Timeline::new();
    timeline
        .bind_attribute(
            &mut scene,
            attribute,
            AttributeValues::Scalar(Arc::from([0.5, 0.75])),
            AttributeValues::Scalar(Arc::from([1.5, 1.25])),
            warp,
        )
        .unwrap_or_else(|error| panic!("{error}"));

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let glyph = engine
        .device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .find(|(_, label, _)| *label == "interaction glyph table")
        .map_or_else(|| panic!("glyph table exists"), |(id, _, _)| *id);
    let glyph_writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .filter(|(id, _, _, _)| *id == glyph)
        .count();
    timeline
        .apply(&mut scene, 0.5)
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        writes.iter().filter(|(id, _, _, _)| *id == glyph).count(),
        glyph_writes,
        "timeline sampling updates only the attribute arena and visual uniform"
    );
}

#[test]
fn static_relation_picking_restores_the_external_source_key() {
    let mut scene = Scene::new();
    let relations = Arc::from([
        Relation {
            start: SpatialAnchor::world(Vec3::ZERO).unwrap_or_else(|error| panic!("{error}")),
            end: SpatialAnchor::world(Vec3::X).unwrap_or_else(|error| panic!("{error}")),
        },
        Relation {
            start: SpatialAnchor::world(Vec3::Y).unwrap_or_else(|error| panic!("{error}")),
            end: SpatialAnchor::world(Vec3::ONE).unwrap_or_else(|error| panic!("{error}")),
        },
    ]);
    let rows = SourceRows::keyed(SourceNamespace(711), Arc::from([17_u64, 88]))
        .unwrap_or_else(|error| panic!("{error}"));
    let batch = RelationBatch::new(relations, rows, RelationStyle::default())
        .unwrap_or_else(|error| panic!("{error}"));
    let handle = scene
        .add_relation_batch(batch)
        .unwrap_or_else(|error| panic!("{error}"));
    let chunk = ChunkId::new(u64::from(handle.row()) | (u64::from(handle.generation()) << 32));

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .capture_pick_submission_for_test()
        .unwrap_or_else(|error| panic!("{error}"));
    let page = engine
        .scene_gpu
        .test_table_pick_page(DatasetId::new(711), chunk, EntityKind::Relation)
        .unwrap_or_else(|| panic!("generic relation page is resident"));
    let identity = engine
        .resolve_pick_token_for_test(GpuPickToken::new(page, 1))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(identity.dataset(), DatasetId::new(711));
    assert_eq!(identity.chunk(), chunk);
    assert_eq!(identity.kind(), EntityKind::Relation);
    assert_eq!(identity.row(), LogicalRow::new(88));
}

#[test]
fn dynamic_point_relations_resolve_once_without_cpu_endpoint_uploads() {
    let mut scene = Scene::new();
    let points = scene.add_point_batch(
        PointBatch::new(
            Arc::from([[0.0, 0.0, 0.0], [1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]),
            SourceRows::ordered(SourceNamespace(712), 3),
            PointGlyph::Sphere,
            PointStyle::default(),
        )
        .unwrap_or_else(|error| panic!("points validate: {error}")),
    );
    let point = |row| {
        SpatialAnchor::entity(RowEntityRef::new(RowDomain::Points(points), row))
            .unwrap_or_else(|error| panic!("point anchor validates: {error}"))
    };
    let relations = Arc::from([
        Relation {
            start: point(0),
            end: point(1),
        },
        Relation {
            start: point(1),
            end: point(2),
        },
    ]);
    let batch = RelationBatch::new(
        relations,
        SourceRows::ordered(SourceNamespace(713), 2),
        RelationStyle::default(),
    )
    .unwrap_or_else(|error| panic!("relations validate: {error}"));
    scene
        .add_relation_batch(batch)
        .unwrap_or_else(|error| panic!("relations attach: {error}"));

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("dynamic relations render: {error}"));
    let first_dispatches = engine
        .device
        .log
        .dispatches
        .lock()
        .map_or_else(|error| panic!("{error}"), |dispatches| dispatches.len());
    assert_eq!(
        first_dispatches, 6,
        "one resolver, three point LOD and two relation cull dispatches run"
    );
    let writes_before = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("stable dynamic relations render: {error}"));
    let dispatches = engine
        .device
        .log
        .dispatches
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        &dispatches[first_dispatches..],
        &[(250, 1, 1), (1, 1, 1), (250, 1, 1), (1, 1, 1), (1, 1, 1),],
        "stable endpoints skip resolution while visibility still follows the camera"
    );
    drop(dispatches);
    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        writes.len() - writes_before,
        1,
        "stable frames upload only frame uniforms"
    );
}

#[test]
fn atom_relations_follow_model_changes_without_reuploading_endpoints() {
    let mut scene = Scene::new();
    let structure = scene
        .add_structure(&structure())
        .unwrap_or_else(|error| panic!("structure attaches: {error}"));
    let atom = |row| {
        SpatialAnchor::entity(RowEntityRef::new(RowDomain::Atoms(structure), row))
            .unwrap_or_else(|error| panic!("atom anchor validates: {error}"))
    };
    let batch = RelationBatch::new(
        Arc::from([Relation {
            start: atom(0),
            end: atom(1),
        }]),
        SourceRows::ordered(SourceNamespace(714), 1),
        RelationStyle::default(),
    )
    .unwrap_or_else(|error| panic!("relations validate: {error}"));
    scene
        .add_relation_batch(batch)
        .unwrap_or_else(|error| panic!("relations attach: {error}"));

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("atom relations render: {error}"));
    let glyph_buffer = engine
        .device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .find(|(_, label, _)| *label == "interaction glyph table")
        .map_or_else(|| panic!("glyph buffer exists"), |(id, _, _)| *id);
    let glyph_writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .filter(|(id, _, _, _)| *id == glyph_buffer)
        .count();
    scene
        .structure_mut(structure)
        .unwrap_or_else(|| panic!("structure stays live"))
        .model_to_world = Mat4::from_translation(Vec3::new(3.0, 4.0, 5.0));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("moved atom relations render: {error}"));
    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        writes
            .iter()
            .filter(|(id, _, _, _)| *id == glyph_buffer)
            .count(),
        glyph_writes,
        "model motion updates the bound transform, never relation endpoints"
    );
    drop(writes);
    let dispatches = engine
        .device
        .log
        .dispatches
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        dispatches.as_slice(),
        &[
            (1, 1, 1),
            (1, 1, 1),
            (1, 1, 1),
            (1, 1, 1),
            (1, 1, 1),
            (1, 1, 1),
        ],
        "each frame culls on GPU and the atom stream re-resolves only after model motion"
    );
}

#[test]
fn template_part_relations_resolve_the_local_part_through_its_rigid_instance() {
    let mut scene = Scene::new();
    let template = Arc::new(
        AnalyticTemplate::new(
            Arc::from([AnalyticSphere {
                center: [1.0, 0.0, 0.0],
                radius: 0.5,
            }]),
            Arc::from([]),
            SourceRows::ordered(SourceNamespace(715), 1),
        )
        .unwrap_or_else(|error| panic!("template validates: {error}")),
    );
    let instances = InstanceBatch::new(
        template,
        Arc::from([
            RigidInstance::new(Vec3::new(2.0, 0.0, 0.0), Quat::IDENTITY, 2.0)
                .unwrap_or_else(|error| panic!("transform validates: {error}")),
        ]),
        SourceRows::ordered(SourceNamespace(716), 1),
    )
    .unwrap_or_else(|error| panic!("instances validate: {error}"));
    let instances = scene.add_instance_batch(instances);
    let batch = RelationBatch::new(
        Arc::from([Relation {
            start: SpatialAnchor::world(Vec3::ZERO)
                .unwrap_or_else(|error| panic!("world anchor validates: {error}")),
            end: SpatialAnchor::template_part(TemplatePartRef::new(instances, 0, 0)),
        }]),
        SourceRows::ordered(SourceNamespace(717), 1),
        RelationStyle::default(),
    )
    .unwrap_or_else(|error| panic!("relations validate: {error}"));
    scene
        .add_relation_batch(batch)
        .unwrap_or_else(|error| panic!("relations attach: {error}"));

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("template relation renders: {error}"));
    let dispatches = engine
        .device
        .log
        .dispatches
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        dispatches.as_slice(),
        &[(1, 1, 1), (1, 1, 1), (1, 1, 1), (1, 1, 1), (1, 1, 1),],
        "one rigid resolver precedes instance and relation reset/culling"
    );
}
