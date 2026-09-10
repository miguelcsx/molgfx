use super::tests::{camera, engine};
use super::{DerivedCacheBudget, Engine, EngineConfig};
use crate::testing::MockDevice;
use pdviewx_core::{
    AnalyticCapsule, AnalyticSphere, AnalyticTemplate, AttributeColumn, AttributeValues, ChunkId,
    DatasetId, EntityKind, GpuPickToken, InstanceBatch, PlaybackMode, RigidInstance, RowDomain,
    Scene, SourceNamespace, SourceRows, TimeWarp, Timeline, VisualDescriptor, VisualProgramBuilder,
    VisualStyle,
};
use pdviewx_math::{Quat, Rgba8, Vec3};
use std::sync::Arc;

fn template(namespace: u64) -> Arc<AnalyticTemplate> {
    Arc::new(
        AnalyticTemplate::new(
            Arc::from([AnalyticSphere {
                center: [0.0; 3],
                radius: 0.5,
            }]),
            Arc::from([AnalyticCapsule::new(Vec3::ZERO, Vec3::Z, 0.2)
                .unwrap_or_else(|error| panic!("{error}"))]),
            SourceRows::keyed(SourceNamespace(namespace), Arc::from([31_u64, 47]))
                .unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}")),
    )
}

fn transforms(count: u16) -> Arc<[RigidInstance]> {
    (0..count)
        .map(|row| {
            RigidInstance::new(
                Vec3::new(f32::from(row) * 0.01, 0.0, 0.0),
                Quat::IDENTITY,
                1.0,
            )
            .unwrap_or_else(|error| panic!("{error}"))
        })
        .collect::<Vec<_>>()
        .into()
}

#[test]
fn timeline_materialization_is_a_real_budgeted_optional_allocation() {
    let mut scene = Scene::new();
    let batch = InstanceBatch::new(
        template(731),
        transforms(4),
        SourceRows::ordered(SourceNamespace(732), 4),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let handle = scene.add_instance_batch(batch);
    let start = transforms(4);
    let end: Arc<[RigidInstance]> = start
        .iter()
        .map(|value| {
            RigidInstance::new(
                value.translation() + Vec3::Y,
                Quat::from_array(value.orientation),
                value.scale(),
            )
            .unwrap_or_else(|error| panic!("{error}"))
        })
        .collect::<Vec<_>>()
        .into();
    let warp = TimeWarp::new(0.0, 0.0, 1.0, [0.0, 1.0], PlaybackMode::Clamp)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut timeline = Timeline::new();
    timeline
        .bind_instances(&mut scene, handle, start, end, warp)
        .unwrap_or_else(|error| panic!("{error}"));
    timeline
        .apply(&mut scene, 0.5)
        .unwrap_or_else(|error| panic!("{error}"));

    let direct_config = EngineConfig {
        derived_cache: DerivedCacheBudget {
            cpu_bytes: 0,
            gpu_bytes: 0,
        },
        ..EngineConfig::default()
    };
    let mut direct =
        Engine::<MockDevice>::new(&direct_config, None).unwrap_or_else(|error| panic!("{error}"));
    direct
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let direct_buffers = direct
        .device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        !direct_buffers
            .iter()
            .any(|(_, label, _)| *label == "materialized generic instance timeline")
    );
    drop(direct_buffers);

    let mut cached = engine();
    cached
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let cached_buffers = cached
        .device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        cached_buffers
            .iter()
            .any(|(_, label, _)| *label == "materialized generic instance timeline")
    );
    assert_eq!(cached.derived_cache_usage().gpu_bytes, 4 * 32);
}

#[test]
fn four_hundred_instances_are_gpu_culled_then_expand_two_homogeneous_draws() {
    assert_eq!(std::mem::size_of::<RigidInstance>(), 32);
    let mut scene = Scene::new();
    let batch = InstanceBatch::new(
        template(720),
        transforms(400),
        SourceRows::ordered(SourceNamespace(721), 400),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let _handle = scene.add_instance_batch(batch);

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
    assert_eq!(dispatches.as_slice(), &[(1, 1, 1), (7, 1, 1)]);
    drop(dispatches);
    let draws = engine
        .device
        .log
        .indirect_draws
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(draws.len(), 2, "sphere and capsule stay homogeneous");
    assert_eq!(draws[0].1, 0);
    assert_eq!(draws[1].1, 16);
}

#[test]
fn tiny_instance_batches_reserve_the_full_runtime_storage_struct() {
    for count in 1..=4 {
        let mut scene = Scene::new();
        let batch = InstanceBatch::new(
            template(799),
            transforms(count),
            SourceRows::ordered(SourceNamespace(798), u32::from(count)),
        )
        .unwrap_or_else(|error| panic!("instances validate: {error}"));
        let _ = scene.add_instance_batch(batch);
        let mut engine = engine();
        engine
            .render(&scene, &camera())
            .unwrap_or_else(|error| panic!("instances render: {error}"));
        let buffers = engine
            .device
            .log
            .buffers
            .lock()
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(
            buffers.iter().any(|(_, label, bytes)| {
                *label == "generic instance cull output" && *bytes == 48
            })
        );
    }
}

#[test]
fn sphere_only_templates_still_bind_one_complete_capsule_record() {
    let template = AnalyticTemplate::new(
        Arc::from([AnalyticSphere {
            center: [0.0; 3],
            radius: 0.5,
        }]),
        Arc::from([]),
        SourceRows::ordered(SourceNamespace(795), 1),
    )
    .unwrap_or_else(|error| panic!("template validates: {error}"));
    let batch = InstanceBatch::new(
        Arc::new(template),
        transforms(1),
        SourceRows::ordered(SourceNamespace(794), 1),
    )
    .unwrap_or_else(|error| panic!("batch validates: {error}"));
    let mut scene = Scene::new();
    let _ = scene.add_instance_batch(batch);
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("renders: {error}"));
    let buffers = engine
        .device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        buffers.iter().any(|(_, label, size)| {
            *label == "generic analytic template capsules" && *size == 48
        })
    );
}

#[test]
fn instance_visuals_split_geometry_before_culling_and_appearance_after_compaction() {
    let mut scene = Scene::new();
    let batch = InstanceBatch::new(
        template(727),
        transforms(130),
        SourceRows::ordered(SourceNamespace(728), 130),
    )
    .unwrap_or_else(|error| panic!("instances validate: {error}"));
    let handle = scene.add_instance_batch(batch);
    let domain = RowDomain::Instances(handle);
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
        .unwrap_or_else(|error| panic!("visual instances render: {error}"));
    let dispatches = engine
        .device
        .log
        .dispatches
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        dispatches.as_slice(),
        &[(3, 1, 1), (1, 1, 1), (3, 1, 1), (3, 1, 1)],
        "geometry runs before instance culling and appearance after compaction",
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
fn shared_templates_upload_once_and_stable_frames_only_upload_uniforms() {
    let shared = template(722);
    let mut scene = Scene::new();
    for namespace in [723, 724] {
        let batch = InstanceBatch::new(
            Arc::clone(&shared),
            transforms(2),
            SourceRows::ordered(SourceNamespace(namespace), 2),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let _handle = scene.add_instance_batch(batch);
    }
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let template_buffers = engine
        .device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .filter(|(_, label, _)| *label == "generic analytic template spheres")
        .count();
    assert_eq!(template_buffers, 1);
    let before = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let after = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());
    assert_eq!(
        after - before,
        1,
        "stable frame changes only frame uniforms"
    );
}

#[test]
fn template_part_picking_uses_one_flat_attachment_index() {
    let mut scene = Scene::new();
    let batch = InstanceBatch::new(
        template(725),
        transforms(2),
        SourceRows::keyed(SourceNamespace(726), Arc::from([101_u64, 303]))
            .unwrap_or_else(|error| panic!("{error}")),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let handle = scene.add_instance_batch(batch);
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
        .test_table_pick_page(DatasetId::new(725), chunk, EntityKind::TemplatePart)
        .unwrap_or_else(|| panic!("template-part occurrence page is resident"));
    let identity = engine
        .resolve_pick_token_for_test(GpuPickToken::new(page, 3))
        .unwrap_or_else(|error| panic!("{error}"));
    let resolved = scene
        .resolve_template_part_pick(identity)
        .unwrap_or_else(|| panic!("flattened occurrence resolves"));
    assert_eq!(resolved.reference().instance_row(), 1);
    assert_eq!(resolved.reference().part_row(), 1);
    assert_eq!(resolved.instance_source_key(), 303);
    assert_eq!(resolved.part_source_key(), 47);
}

#[test]
fn translucent_generic_instances_use_only_the_oit_draw_route() {
    let mut scene = Scene::new();
    let batch = InstanceBatch::new(
        template(729),
        transforms(2),
        SourceRows::ordered(SourceNamespace(730), 2),
    )
    .unwrap_or_else(|error| panic!("instances validate: {error}"))
    .with_style(pdviewx_core::InstanceStyle {
        color: Rgba8::new(40, 180, 220, 128),
    });
    let _handle = scene.add_instance_batch(batch);
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("transparent instances render: {error}"));

    assert_eq!(engine.scene_gpu.generic_instance_draws(false).count(), 0);
    assert_eq!(engine.scene_gpu.generic_instance_draws(true).count(), 2);
    assert!(engine.scene_gpu.has_translucency());
}
