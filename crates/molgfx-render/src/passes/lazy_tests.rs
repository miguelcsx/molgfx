use super::Lazy;
use crate::error::RenderError;
use std::cell::Cell;

#[test]
fn a_value_is_built_once_however_often_it_is_asked_for() {
    let builds = Cell::new(0);
    let lazy = Lazy::default();
    for _ in 0..3 {
        let value = lazy.get_or_build(|| {
            builds.set(builds.get() + 1);
            Ok(7)
        });
        assert_eq!(value.copied().ok(), Some(7));
    }
    assert_eq!(builds.get(), 1);
}

#[test]
fn nothing_is_built_until_it_is_asked_for() {
    let lazy: Lazy<u32> = Lazy::default();
    assert!(lazy.get().is_none());
}

#[test]
fn a_failed_build_leaves_the_value_unbuilt_so_a_later_frame_retries() {
    let lazy: Lazy<u32> = Lazy::default();
    let failed = lazy.get_or_build(|| {
        Err(RenderError::Residency {
            reason: "pipeline did not compile",
        })
    });
    assert!(failed.is_err());
    assert!(lazy.get().is_none());
    assert_eq!(lazy.get_or_build(|| Ok(3)).copied().ok(), Some(3));
}
