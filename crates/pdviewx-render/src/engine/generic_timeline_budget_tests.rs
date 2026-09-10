use super::tests::camera;
use super::{DerivedCacheBudget, Engine, EngineConfig};
use crate::testing::MockDevice;
use pdviewx_core::{
    AnalyticSphere, AnalyticTemplate, AttributeColumn, AttributeValues, InstanceBatch,
    PlaybackMode, PointBatch, PointGlyph, PointStyle, Relation, RelationBatch, RelationStyle,
    RigidInstance, RowDomain, RowEntityRef, Scene, SourceNamespace, SourceRows, SpatialAnchor,
    TemplatePartRef, TimeWarp, Timeline, VisualDescriptor, VisualProgramBuilder, VisualStyle,
};
use pdviewx_math::{Quat, Vec3};
use std::sync::Arc;

fn template(namespace: u64) -> Arc<AnalyticTemplate> {
    Arc::new(
        AnalyticTemplate::new(
            Arc::from([AnalyticSphere {
                center: [0.5, 0.0, 0.0],
                radius: 0.25,
            }]),
            Arc::from([]),
            SourceRows::ordered(SourceNamespace(namespace), 1),
        )
        .unwrap_or_else(|error| panic!("template validates: {error}")),
    )
}

fn transforms(count: u16, y: f32) -> Arc<[RigidInstance]> {
    (0..count)
        .map(|row| {
            RigidInstance::new(Vec3::new(f32::from(row), y, 0.0), Quat::IDENTITY, 1.0)
                .unwrap_or_else(|error| panic!("transform validates: {error}"))
        })
        .collect::<Vec<_>>()
        .into()
}

fn warp() -> TimeWarp {
    TimeWarp::new(0.0, 0.0, 1.0, [0.0, 1.0], PlaybackMode::Clamp)
        .unwrap_or_else(|error| panic!("warp validates: {error}"))
}

#[test]
fn zero_budget_relations_sample_instance_frames_without_a_derived_allocation() {
    let mut scene = Scene::new();
    let instances = InstanceBatch::new(
        template(740),
        transforms(2, 0.0),
        SourceRows::ordered(SourceNamespace(741), 2),
    )
    .unwrap_or_else(|error| panic!("instances validate: {error}"));
    let instances = scene.add_instance_batch(instances);
    scene
        .add_relation_batch(
            RelationBatch::new(
                Arc::from([Relation {
                    start: SpatialAnchor::world(Vec3::ZERO)
                        .unwrap_or_else(|error| panic!("anchor validates: {error}")),
                    end: SpatialAnchor::template_part(TemplatePartRef::new(instances, 0, 0)),
                }]),
                SourceRows::ordered(SourceNamespace(742), 1),
                RelationStyle::default(),
            )
            .unwrap_or_else(|error| panic!("relations validate: {error}")),
        )
        .unwrap_or_else(|error| panic!("relations attach: {error}"));
    let mut timeline = Timeline::new();
    timeline
        .bind_instances(
            &mut scene,
            instances,
            transforms(2, 0.0),
            transforms(2, 2.0),
            warp(),
        )
        .unwrap_or_else(|error| panic!("timeline binds: {error}"));
    timeline
        .apply(&mut scene, 0.25)
        .unwrap_or_else(|error| panic!("timeline samples: {error}"));

    let config = EngineConfig {
        derived_cache: DerivedCacheBudget {
            cpu_bytes: 0,
            gpu_bytes: 0,
        },
        ..EngineConfig::default()
    };
    let mut engine = Engine::<MockDevice>::new(&config, None)
        .unwrap_or_else(|error| panic!("engine opens: {error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("first frame renders: {error}"));
    timeline
        .apply(&mut scene, 0.75)
        .unwrap_or_else(|error| panic!("timeline resamples: {error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("second frame renders: {error}"));

    assert_eq!(engine.derived_cache_usage().gpu_bytes, 0);
    let buffers = engine
        .device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("buffer log locks: {error}"));
    assert!(
        !buffers
            .iter()
            .any(|(_, label, _)| { *label == "materialized generic instance timeline" })
    );
    drop(buffers);
    let dispatches = engine
        .device
        .log
        .dispatches
        .lock()
        .unwrap_or_else(|error| panic!("dispatch log locks: {error}"));
    assert_eq!(
        dispatches
            .iter()
            .filter(|groups| **groups == (1, 1, 1))
            .count(),
        10,
        "the rigid relation resolves again after the GPU timeline sample changes"
    );
}

#[test]
fn deformable_points_and_their_relations_share_one_gpu_timeline() {
    let mut scene = Scene::new();
    let start: Arc<[[f32; 3]]> = Arc::from([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
    let end: Arc<[[f32; 3]]> = Arc::from([[0.0, 0.2, 0.0], [1.0, -0.2, 0.0]]);
    let points = scene.add_point_batch(
        PointBatch::new(
            Arc::clone(&start),
            SourceRows::ordered(SourceNamespace(748), 2),
            PointGlyph::Sphere,
            PointStyle::default(),
        )
        .unwrap_or_else(|error| panic!("points validate: {error}")),
    );
    let anchor = |row| {
        SpatialAnchor::entity(RowEntityRef::new(RowDomain::Points(points), row))
            .unwrap_or_else(|error| panic!("anchor validates: {error}"))
    };
    scene
        .add_relation_batch(
            RelationBatch::new(
                Arc::from([Relation {
                    start: anchor(0),
                    end: anchor(1),
                }]),
                SourceRows::ordered(SourceNamespace(749), 1),
                RelationStyle::default(),
            )
            .unwrap_or_else(|error| panic!("relations validate: {error}")),
        )
        .unwrap_or_else(|error| panic!("relations attach: {error}"));
    let mut timeline = Timeline::new();
    timeline
        .bind_points(&mut scene, points, start, end, warp())
        .unwrap_or_else(|error| panic!("timeline binds: {error}"));
    timeline
        .apply(&mut scene, 0.25)
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
        .unwrap_or_else(|error| panic!("direct frame renders: {error}"));
    assert_eq!(direct.derived_cache_usage().gpu_bytes, 0);
    assert!(!has_buffer(&direct, "materialized generic point timeline"));
    assert_eq!(latest_point_alpha(&direct).to_bits(), 0.25f32.to_bits());

    let mut cached = Engine::<MockDevice>::new(&EngineConfig::default(), None)
        .unwrap_or_else(|error| panic!("cached engine opens: {error}"));
    cached
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("cached frame renders: {error}"));
    assert!(has_buffer(&cached, "materialized generic point timeline"));
    assert_eq!(cached.derived_cache_usage().gpu_bytes, 24);
    assert_eq!(latest_point_alpha(&cached).to_bits(), 0.0f32.to_bits());
}

fn has_buffer(engine: &Engine<MockDevice>, label: &'static str) -> bool {
    engine
        .device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("buffer log locks: {error}"))
        .iter()
        .any(|(_, candidate, _)| *candidate == label)
}

fn latest_point_alpha(engine: &Engine<MockDevice>) -> f32 {
    let id = engine
        .device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("buffer log locks: {error}"))
        .iter()
        .find(|(_, label, _)| *label == "generic point configuration")
        .map_or_else(|| panic!("point config exists"), |(id, _, _)| *id);
    let index = engine.device.log.writes.lock().ok().and_then(|writes| {
        writes
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, (buffer, _, _, _))| (*buffer == id).then_some(index))
    });
    let bytes = index.and_then(|index| {
        engine
            .device
            .log
            .write_payloads
            .lock()
            .ok()
            .and_then(|payloads| payloads.get(index).cloned())
    });
    let Some(bytes) = bytes else {
        panic!("point config is written")
    };
    f32::from_ne_bytes(
        bytes[32..36]
            .try_into()
            .unwrap_or_else(|error| panic!("alpha bytes: {error}")),
    )
}

#[test]
fn one_budget_is_shared_by_attribute_and_instance_materializations() {
    let mut scene = Scene::new();
    let points = scene.add_point_batch(
        PointBatch::new(
            Arc::from(vec![[0.0, 0.0, 0.0]; 4]),
            SourceRows::ordered(SourceNamespace(743), 4),
            PointGlyph::Sphere,
            PointStyle::default(),
        )
        .unwrap_or_else(|error| panic!("points validate: {error}")),
    );
    let domain = RowDomain::Points(points);
    let attribute = scene
        .add_attribute(
            AttributeColumn::new(
                domain,
                "animated",
                AttributeValues::Scalar(Arc::from([1.0; 4])),
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

    let instances = scene.add_instance_batch(
        InstanceBatch::new(
            template(744),
            transforms(4, 0.0),
            SourceRows::ordered(SourceNamespace(745), 4),
        )
        .unwrap_or_else(|error| panic!("instances validate: {error}")),
    );
    let mut timeline = Timeline::new();
    timeline
        .bind_attribute(
            &mut scene,
            attribute,
            AttributeValues::Scalar(Arc::from([0.5; 4])),
            AttributeValues::Scalar(Arc::from([1.5; 4])),
            warp(),
        )
        .unwrap_or_else(|error| panic!("attribute timeline binds: {error}"));
    timeline
        .bind_instances(
            &mut scene,
            instances,
            transforms(4, 0.0),
            transforms(4, 1.0),
            warp(),
        )
        .unwrap_or_else(|error| panic!("instance timeline binds: {error}"));
    timeline
        .apply(&mut scene, 0.5)
        .unwrap_or_else(|error| panic!("timelines sample: {error}"));

    let config = EngineConfig {
        derived_cache: DerivedCacheBudget {
            cpu_bytes: 0,
            gpu_bytes: 128,
        },
        ..EngineConfig::default()
    };
    let mut engine = Engine::<MockDevice>::new(&config, None)
        .unwrap_or_else(|error| panic!("engine opens: {error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("scene renders: {error}"));

    let usage = engine.derived_cache_usage();
    assert_eq!(usage.gpu_bytes, 16, "the first deterministic plan wins");
    assert!(usage.peak_gpu_bytes <= config.derived_cache.gpu_bytes);
    let buffers = engine
        .device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("buffer log locks: {error}"));
    assert!(
        !buffers
            .iter()
            .any(|(_, label, _)| { *label == "materialized generic instance timeline" })
    );
}
