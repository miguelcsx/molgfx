use super::*;

#[test]
fn ray_query_is_absent_from_the_portable_default() {
    let capabilities = Capabilities::default();

    assert!(!capabilities.ray_query());
    assert!(!capabilities.hardware_ray_tracing());
}

#[test]
fn hardware_ray_tracing_reports_the_ray_query_capability() {
    let capabilities = Capabilities {
        flags: CapabilityFlags::RAY_QUERY,
        ..Capabilities::default()
    };

    assert!(capabilities.ray_query());
    assert!(capabilities.hardware_ray_tracing());
}
