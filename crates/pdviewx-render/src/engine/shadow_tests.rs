use super::*;
use crate::engine::tests::{camera, structure};

#[test]
fn shadow_bounds_refresh_only_for_a_changed_or_replaced_scene() {
    let mut cache = ShadowBoundCache::default();
    let mut scene = Scene::new();
    let view = camera();
    cache.fit(&scene, &view, LightingEnvironment::default(), false);
    assert!(cache.bound().is_empty());

    let source = structure();
    if let Err(error) = scene.add_structure(&source) {
        panic!("fixture structure places: {error}");
    }
    cache.fit(&scene, &view, LightingEnvironment::default(), false);
    assert!(
        cache.bound().is_empty(),
        "stable frames reuse the cached bound"
    );
    cache.fit(&scene, &view, LightingEnvironment::default(), true);
    assert!(!cache.bound().is_empty());

    let replacement = Scene::new();
    cache.fit(&replacement, &view, LightingEnvironment::default(), false);
    assert!(
        cache.bound().is_empty(),
        "scene identity invalidates the cache"
    );
}
