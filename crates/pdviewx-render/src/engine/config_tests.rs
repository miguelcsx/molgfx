use super::{EngineConfig, RenderMode};

#[test]
fn realtime_is_the_portable_default_render_mode() {
    assert_eq!(EngineConfig::default().mode, RenderMode::Realtime);
}

#[test]
fn inspection_profile_is_the_portable_default() {
    assert!(EngineConfig::default().profile.layers().is_empty());
}
