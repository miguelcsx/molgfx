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
    assert!(
        engine
            .pass_nodes
            .iter()
            .all(|node| !node.name.starts_with("depth-of-field"))
    );

    if let Err(error) = engine.set_render_profile(RenderProfile::cinematic()) {
        panic!("cinematic profile resolves: {error}")
    }

    // Cinematic contributes two depth-of-field stages, one motion gather and
    // three bloom stages.
    assert_eq!(engine.pass_nodes.len(), baseline_nodes + 6);
    assert!(
        engine
            .pass_nodes
            .iter()
            .any(|node| node.name == "bloom vertical blur")
    );
    assert_eq!(
        engine.resolved_render_plan().depth_of_field(),
        Some(crate::engine::DepthOfField::cinematic())
    );
    assert!(
        engine
            .pass_nodes
            .iter()
            .any(|node| node.name == "depth-of-field bounded gather")
    );
    assert!(
        engine
            .pass_nodes
            .iter()
            .any(|node| node.name == "camera-shutter motion blur")
    );
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
    assert!(
        engine
            .pass_nodes
            .iter()
            .all(|node| !node.name.starts_with("depth-of-field"))
    );
}
