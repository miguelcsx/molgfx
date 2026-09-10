use super::{EngineConfig, RenderMode};

#[test]
fn realtime_is_the_portable_default_render_mode() {
    assert_eq!(EngineConfig::default().mode, RenderMode::Realtime);
}

#[test]
fn inspection_profile_is_the_portable_default() {
    assert!(EngineConfig::default().profile.layers().is_empty());
}

#[test]
fn physical_gpu_memory_is_unbounded_until_the_embedder_sets_a_limit() {
    assert_eq!(EngineConfig::default().resource_memory_limit_bytes, None);
}
