use super::*;
use crate::testing::MockDevice;

fn engine() -> Engine<MockDevice> {
    match Engine::new(&crate::engine::EngineConfig::default(), None) {
        Ok(engine) => engine,
        Err(error) => panic!("mock engine opens: {error}"),
    }
}

#[test]
fn topology_modules_rebuild_the_graph_without_reopening_the_device() {
    let mut engine = engine();
    let baseline_nodes = engine.pass_nodes.len();
    assert!(engine
        .pass_nodes
        .iter()
        .all(|node| !node.name.starts_with("depth-of-field")));

    if let Err(error) = engine.set_render_profile(RenderProfile::cinematic()) {
        panic!("cinematic profile resolves: {error}")
    }

    // Cinematic contributes two depth-of-field stages, one motion gather and
    // three bloom stages.
    assert_eq!(engine.pass_nodes.len(), baseline_nodes + 6);
    assert!(engine
        .pass_nodes
        .iter()
        .any(|node| node.name == "bloom vertical blur"));
    assert_eq!(
        engine.resolved_render_plan().depth_of_field(),
        Some(crate::engine::DepthOfField::cinematic())
    );
    assert!(engine
        .pass_nodes
        .iter()
        .any(|node| node.name == "depth-of-field bounded gather"));
    assert!(engine
        .pass_nodes
        .iter()
        .any(|node| node.name == "camera-shutter motion blur"));
}

#[test]
fn asynchronous_construction_uses_the_same_resolved_graph() {
    let opened = pollster::block_on(Engine::<MockDevice>::new_async(
        &crate::engine::EngineConfig::default(),
        None,
    ));
    let engine = match opened {
        Ok(engine) => engine,
        Err(error) => panic!("mock engine opens asynchronously: {error}"),
    };
    assert!(engine.resolved_render_plan().depth_of_field().is_none());
    assert!(engine
        .pass_nodes
        .iter()
        .all(|node| !node.name.starts_with("depth-of-field")));
}

#[test]
fn every_effect_combination_allocates_only_its_active_targets_and_bindings() {
    use crate::engine::{BloomStyle, DepthOfField, MotionBlur, PresentationEffect};
    use crate::passes::{BLOOM_A_RESOURCE, DOF_TILE_RESOURCE, MOTION_BLUR_RESOURCE};
    let mut engine = engine();
    let scene = molgfx_core::Scene::new();
    for mask in [0, 1, 2, 3, 4, 5, 6, 7, 0] {
        let mut profile = RenderProfile::inspection();
        if mask & 1 != 0 {
            profile =
                profile.with_effect(PresentationEffect::DepthOfField(DepthOfField::cinematic()));
        }
        if mask & 2 != 0 {
            profile = profile.with_effect(PresentationEffect::Bloom(BloomStyle::cinematic()));
        }
        if mask & 4 != 0 {
            profile = profile.with_effect(PresentationEffect::MotionBlur(MotionBlur::cinematic()));
        }
        engine.set_render_profile(profile).expect("profile applies");
        engine
            .render(&scene, &crate::engine::tests::camera())
            .expect("frame renders");
        let pool = engine.pool.as_ref().expect("frame pool exists");
        let bindings = engine
            .bindings
            .as_ref()
            .expect("complete frame bindings exist");
        assert_eq!(pool.view(DOF_TILE_RESOURCE).is_some(), mask & 1 != 0);
        assert_eq!(pool.view(BLOOM_A_RESOURCE).is_some(), mask & 2 != 0);
        assert_eq!(pool.view(MOTION_BLUR_RESOURCE).is_some(), mask & 4 != 0);
        assert_eq!(bindings.dof.is_some(), mask & 1 != 0);
        assert_eq!(bindings.bloom_source.is_some(), mask & 2 != 0);
        assert_eq!(bindings.motion_blur.is_some(), mask & 4 != 0);
        assert_eq!(engine.passes.depth_of_field.is_some(), mask & 1 != 0);
        assert_eq!(engine.passes.bloom.is_some(), mask & 2 != 0);
        assert_eq!(engine.passes.motion_blur.is_some(), mask & 4 != 0);
        assert!(bindings.tonemap.get(0).is_some());
        assert!(bindings.tonemap.get(1).is_some());
        let textures = engine.device.log.textures.lock().expect("log locks").len();
        let buffers = engine.device.log.buffers.lock().expect("log locks").len();
        engine
            .render(&scene, &crate::engine::tests::camera())
            .expect("stable frame renders");
        assert_eq!(
            engine.device.log.textures.lock().expect("log locks").len(),
            textures
        );
        assert_eq!(
            engine.device.log.buffers.lock().expect("log locks").len(),
            buffers
        );
    }
}
