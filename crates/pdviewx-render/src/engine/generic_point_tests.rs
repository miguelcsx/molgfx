use super::tests::{camera, engine};
use super::{DerivedCacheBudget, Engine, EngineConfig};
use crate::testing::MockDevice;
use pdviewx_core::{
    AnalyticSphere, AnalyticTemplate, AttributeColumn, AttributeValues, ChunkId, DatasetId,
    EntityKind, GpuPickToken, InstanceBatch, LogicalRow, PlaybackMode, PointBatch, PointGlyph,
    PointStyle, RigidInstance, RowDomain, Scene, SourceNamespace, SourceRows, TimeWarp, Timeline,
    VisualDescriptor, VisualProgramBuilder, VisualStyle,
};
use pdviewx_math::{Quat, Rgba8, Vec3};
use std::sync::Arc;

#[test]
fn generic_points_keep_twelve_byte_positions_and_use_compute_indirect_draw() {
    assert_eq!(std::mem::size_of::<[f32; 3]>(), 12);
    let mut scene = Scene::new();
    let positions = (0_u16..130)
        .map(|index| [f32::from(index) * 0.01, 0.0, 0.0])
        .collect::<Vec<_>>();
    let rows = SourceRows::ordered(SourceNamespace(700), 130);
    let batch = PointBatch::new(
        Arc::from(positions),
        rows,
        PointGlyph::Sphere,
        PointStyle {
            radius: 0.1,
            color: Rgba8::opaque(80, 170, 240),
        },
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let _handle = scene.add_point_batch(batch);

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let dispatches = engine
        .device
        .log
        .dispatches
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        dispatches.as_slice(),
        &[(250, 1, 1), (3, 1, 1), (250, 1, 1)],
        "tile reset, source binning and tile compaction stay GPU-authored",
    );
    drop(dispatches);
    let draws = engine
        .device
        .log
        .indirect_draws
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(draws.len(), 1, "one point batch is one indirect draw");
    let draw_buffer = draws[0].0;
    drop(draws);
    let buffers = engine
        .device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    let output = buffers
        .iter()
        .find(|(_, label, _)| *label == "generic point cull output")
        .unwrap_or_else(|| panic!("packed point output exists"));
    assert_eq!(
        output.0, draw_buffer,
        "the cull output is the indirect source"
    );
    assert_eq!(
        output.2,
        (16 + 130 * 4_u64).next_multiple_of(16),
        "visible rows follow the 16-byte command in an aligned storage struct"
    );
    let output_id = output.0;
    drop(buffers);
    let bindings = engine
        .device
        .log
        .buffer_bindings
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    for label in [
        "group1: generic point culling",
        "group2: generic point rendering",
    ] {
        assert!(
            bindings
                .iter()
                .any(|entry| entry.0 == label && entry.1 == 1 && entry.2 == output_id),
            "{label} shares the packed cull output"
        );
    }
}

#[test]
fn tiny_point_batches_reserve_the_full_runtime_storage_struct() {
    for count in 1_u16..=4 {
        let mut scene = Scene::new();
        let batch = PointBatch::new(
            Arc::from(vec![[0.0, 0.0, 0.0]; usize::from(count)]),
            SourceRows::ordered(SourceNamespace(799), u32::from(count)),
            PointGlyph::Sphere,
            PointStyle::default(),
        )
        .unwrap_or_else(|error| panic!("points validate: {error}"));
        let _ = scene.add_point_batch(batch);
        let mut engine = engine();
        engine
            .render(&scene, &camera())
            .unwrap_or_else(|error| panic!("points render: {error}"));
        let buffers = engine
            .device
            .log
            .buffers
            .lock()
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(
            buffers
                .iter()
                .any(|(_, label, bytes)| { *label == "generic point cull output" && *bytes == 32 })
        );
    }
}

#[test]
fn massive_point_policy_requires_only_opaque_disc_batches() {
    let disc_scene = point_scene(131_072, PointGlyph::Disc);
    let mut disc_engine = engine();
    disc_engine
        .render(&disc_scene, &camera())
        .unwrap_or_else(|error| panic!("discs render: {error}"));
    assert!(disc_engine.scene_gpu.is_massive_points_only());

    let sphere_scene = point_scene(131_072, PointGlyph::Sphere);
    let mut sphere_engine = engine();
    sphere_engine
        .render(&sphere_scene, &camera())
        .unwrap_or_else(|error| panic!("spheres render: {error}"));
    assert!(!sphere_engine.scene_gpu.is_massive_points_only());

    let mut mixed_scene = point_scene(131_072, PointGlyph::Disc);
    let template = AnalyticTemplate::new(
        Arc::from([AnalyticSphere {
            center: [0.0; 3],
            radius: 0.5,
        }]),
        Arc::from([]),
        SourceRows::ordered(SourceNamespace(797), 1),
    )
    .unwrap_or_else(|error| panic!("template validates: {error}"));
    let instances = InstanceBatch::new(
        Arc::new(template),
        Arc::from([RigidInstance::new(Vec3::ZERO, Quat::IDENTITY, 1.0)
            .unwrap_or_else(|error| panic!("instance validates: {error}"))]),
        SourceRows::ordered(SourceNamespace(796), 1),
    )
    .unwrap_or_else(|error| panic!("batch validates: {error}"));
    let _ = mixed_scene.add_instance_batch(instances);
    let mut mixed_engine = engine();
    mixed_engine
        .render(&mixed_scene, &camera())
        .unwrap_or_else(|error| panic!("mixed scene renders: {error}"));
    assert!(!mixed_engine.scene_gpu.is_massive_points_only());
}

fn point_scene(count: usize, glyph: PointGlyph) -> Scene {
    let mut scene = Scene::new();
    let rows = u32::try_from(count).unwrap_or_else(|error| panic!("point count fits: {error}"));
    let batch = PointBatch::new(
        Arc::from(vec![[0.0, 0.0, 0.0]; count]),
        SourceRows::ordered(SourceNamespace(795), rows),
        glyph,
        PointStyle::default(),
    )
    .unwrap_or_else(|error| panic!("points validate: {error}"));
    let _ = scene.add_point_batch(batch);
    scene
}

#[test]
fn massive_discs_do_not_disable_shading_for_resident_spacefill() {
    use super::chunk_residency_tests::{complete, fixture, request_and_deliver, upload};
    use super::{ChunkPlacementId, ChunkRepresentation, StructureChunkPlacement};
    use pdviewx_core::ResidencyOutput;
    use pdviewx_math::Mat4;

    let scene = point_scene(131_072, PointGlyph::Disc);
    let mut engine = super::chunk_residency_tests::engine();
    let mut output = ResidencyOutput::default();
    let (ticket, _, _) = request_and_deliver(&mut engine, fixture(791, 793, 1), &mut output);
    upload(&mut engine, ticket, &mut output);
    complete(&mut engine, &mut output);
    engine
        .set_structure_chunk_placements(&[StructureChunkPlacement {
            id: ChunkPlacementId::new(1),
            ticket,
            model_to_world: Mat4::IDENTITY,
            representation: ChunkRepresentation::spacefill(1.0, Rgba8::WHITE)
                .unwrap_or_else(|error| panic!("spacefill validates: {error}")),
        }])
        .unwrap_or_else(|error| panic!("placement validates: {error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("mixed paged scene renders: {error}"));
    assert!(engine.scene_gpu.paged_spacefill_draw().is_some());
    assert!(!engine.scene_gpu.is_massive_points_only());
}

#[test]
fn generic_point_visuals_split_geometry_before_culling_and_appearance_after_lod() {
    let mut scene = Scene::new();
    let positions = (0_u16..130)
        .map(|index| [f32::from(index) * 0.01, 0.0, 0.0])
        .collect::<Vec<_>>();
    let batch = PointBatch::new(
        Arc::from(positions),
        SourceRows::ordered(SourceNamespace(702), 130),
        PointGlyph::Sphere,
        PointStyle::default(),
    )
    .unwrap_or_else(|error| panic!("points validate: {error}"));
    let handle = scene.add_point_batch(batch);
    let domain = RowDomain::Points(handle);
    let radius = scene
        .add_attribute(
            AttributeColumn::new(
                domain,
                "radius",
                AttributeValues::Scalar(Arc::from(vec![1.0; 130])),
            )
            .unwrap_or_else(|error| panic!("radius validates: {error}")),
        )
        .unwrap_or_else(|error| panic!("radius attaches: {error}"));
    let colors = scene
        .add_attribute(
            AttributeColumn::new(
                domain,
                "color",
                AttributeValues::Color(Arc::from(vec![Rgba8::opaque(40, 180, 220); 130])),
            )
            .unwrap_or_else(|error| panic!("colors validate: {error}")),
        )
        .unwrap_or_else(|error| panic!("colors attach: {error}"));
    let mut builder = VisualProgramBuilder::new();
    let radius = builder
        .scalar_attribute(radius)
        .unwrap_or_else(|error| panic!("radius input builds: {error}"));
    let color = builder
        .color_attribute(colors)
        .unwrap_or_else(|error| panic!("color input builds: {error}"));
    builder
        .set_radius_scale(radius)
        .unwrap_or_else(|error| panic!("radius output builds: {error}"));
    builder
        .set_base_color(color)
        .unwrap_or_else(|error| panic!("color output builds: {error}"));
    scene
        .set_domain_visual(
            domain,
            VisualDescriptor::new(VisualStyle::new(
                builder
                    .finish()
                    .unwrap_or_else(|error| panic!("program builds: {error}")),
            )),
        )
        .unwrap_or_else(|error| panic!("visual attaches: {error}"));

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("visual points render: {error}"));
    let dispatches = engine
        .device
        .log
        .dispatches
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        dispatches.as_slice(),
        &[(3, 1, 1), (250, 1, 1), (3, 1, 1), (250, 1, 1), (3, 1, 1),],
        "geometry runs before point selection and appearance after compaction",
    );
    drop(dispatches);
    let attribute_uploads = engine
        .device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .filter(|(_, label, _)| *label == "visual property table")
        .count();
    assert_eq!(
        attribute_uploads, 1,
        "all typed columns share one GPU arena"
    );
}

#[test]
fn shared_attribute_timeline_materialization_is_budgeted_and_gpu_resident() {
    let mut scene = Scene::new();
    let count = 400_u16;
    let points = PointBatch::new(
        Arc::from(vec![[0.0, 0.0, 0.0]; usize::from(count)]),
        SourceRows::ordered(SourceNamespace(704), u32::from(count)),
        PointGlyph::Sphere,
        PointStyle::default(),
    )
    .unwrap_or_else(|error| panic!("points validate: {error}"));
    let handle = scene.add_point_batch(points);
    let domain = RowDomain::Points(handle);
    let attribute = scene
        .add_attribute(
            AttributeColumn::new(
                domain,
                "animated-scale",
                AttributeValues::Scalar(Arc::from(vec![1.0; usize::from(count)])),
            )
            .unwrap_or_else(|error| panic!("attribute validates: {error}")),
        )
        .unwrap_or_else(|error| panic!("attribute attaches: {error}"));
    let mut builder = VisualProgramBuilder::new();
    let value = builder
        .scalar_attribute(attribute)
        .unwrap_or_else(|error| panic!("attribute input builds: {error}"));
    builder
        .set_radius_scale(value)
        .unwrap_or_else(|error| panic!("radius output builds: {error}"));
    builder
        .set_opacity(value)
        .unwrap_or_else(|error| panic!("opacity output builds: {error}"));
    scene
        .set_domain_visual(
            domain,
            VisualDescriptor::new(VisualStyle::new(
                builder
                    .finish()
                    .unwrap_or_else(|error| panic!("program builds: {error}")),
            )),
        )
        .unwrap_or_else(|error| panic!("visual attaches: {error}"));
    let warp = TimeWarp::new(0.0, 0.0, 1.0, [0.0, 1.0], PlaybackMode::Clamp)
        .unwrap_or_else(|error| panic!("warp validates: {error}"));
    let mut timeline = Timeline::new();
    timeline
        .bind_attribute(
            &mut scene,
            attribute,
            AttributeValues::Scalar(Arc::from(vec![0.5; usize::from(count)])),
            AttributeValues::Scalar(Arc::from(vec![1.5; usize::from(count)])),
            warp,
        )
        .unwrap_or_else(|error| panic!("timeline binds: {error}"));
    timeline
        .apply(&mut scene, 0.5)
        .unwrap_or_else(|error| panic!("timeline samples: {error}"));

    let direct_config = EngineConfig {
        derived_cache: DerivedCacheBudget {
            cpu_bytes: 0,
            gpu_bytes: 0,
        },
        ..EngineConfig::default()
    };
    let mut direct = Engine::<MockDevice>::new(&direct_config, None)
        .unwrap_or_else(|error| panic!("direct engine opens: {error}"));
    direct
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("direct timeline renders: {error}"));
    let direct_size = visual_property_size(&direct);

    let mut cached = engine();
    cached
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("materialized timeline renders: {error}"));
    let cached_size = visual_property_size(&cached);
    assert_eq!(direct_size, 4_096, "direct path stores only two frames");
    assert_eq!(cached_size, 8_192, "cached path owns a real output range");
    assert_eq!(cached.derived_cache_usage().gpu_bytes, u64::from(count) * 4);
}

fn visual_property_size(engine: &Engine<MockDevice>) -> u64 {
    engine
        .device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .filter(|(_, label, _)| *label == "visual property table")
        .map(|(_, _, size)| *size)
        .max()
        .unwrap_or_else(|| panic!("visual property table exists"))
}

#[test]
fn stable_generic_points_upload_only_frame_uniforms_and_picking_restores_source_keys() {
    let mut scene = Scene::new();
    let rows = SourceRows::keyed(SourceNamespace(701), Arc::from([41_u64, 99, 2_000]))
        .unwrap_or_else(|error| panic!("{error}"));
    let batch = PointBatch::new(
        Arc::from([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
        rows,
        PointGlyph::Disc,
        PointStyle::default(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let handle = scene.add_point_batch(batch);
    let chunk = ChunkId::new(u64::from(handle.row()) | (u64::from(handle.generation()) << 32));

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let before = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(writes.len() - before, 1, "only frame uniforms change");
    drop(writes);

    engine
        .capture_pick_submission_for_test()
        .unwrap_or_else(|error| panic!("{error}"));
    let page = engine
        .scene_gpu
        .test_table_pick_page(DatasetId::new(701), chunk, EntityKind::Point)
        .unwrap_or_else(|| panic!("generic point page is resident"));
    let identity = engine
        .resolve_pick_token_for_test(GpuPickToken::new(page, 1))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(identity.dataset(), DatasetId::new(701));
    assert_eq!(identity.chunk(), chunk);
    assert_eq!(identity.kind(), EntityKind::Point);
    assert_eq!(identity.row(), LogicalRow::new(99));
}

#[test]
fn translucent_generic_points_use_only_the_oit_draw_route() {
    let mut scene = Scene::new();
    let batch = PointBatch::new(
        Arc::from([[0.0, 0.0, 0.0]]),
        SourceRows::ordered(SourceNamespace(703), 1),
        PointGlyph::Sphere,
        PointStyle {
            radius: 0.2,
            color: Rgba8::new(40, 180, 220, 128),
        },
    )
    .unwrap_or_else(|error| panic!("points validate: {error}"));
    let _handle = scene.add_point_batch(batch);
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("transparent points render: {error}"));

    assert_eq!(engine.scene_gpu.generic_point_draws(false).count(), 0);
    assert_eq!(engine.scene_gpu.generic_point_draws(true).count(), 1);
    assert!(engine.scene_gpu.has_translucency());
}
