use super::*;
use crate::handle::StructureHandle;

fn owner() -> StructureHandle {
    let mut scene = crate::Scene::new();
    let structure = crate::fixture::structure();
    match scene.add_structure(&structure) {
        Ok(handle) => handle,
        Err(error) => panic!("fixture places: {error:?}"),
    }
}

#[test]
fn a_guide_between_distinct_points_keeps_its_caller_style() {
    let style = GuideStyle {
        color: Rgba8::opaque(10, 200, 120),
        width_pixels: 3.5,
        cap: GuideCap::Arrow,
        ..GuideStyle::default()
    };
    let Ok(guide) = Guide::new(owner(), Vec3::ZERO, Vec3::X * 4.0, style) else {
        panic!("distinct finite endpoints are valid")
    };
    assert_eq!(guide.style().color, Rgba8::opaque(10, 200, 120));
    assert_eq!(guide.style().cap, GuideCap::Arrow);
    assert!((guide.style().width_pixels - 3.5).abs() < f32::EPSILON);
    assert!(guide.visible());
}

#[test]
fn degenerate_or_malformed_endpoints_are_rejected() {
    assert!(Guide::new(owner(), Vec3::ZERO, Vec3::ZERO, GuideStyle::default()).is_err());
    assert!(
        Guide::new(
            owner(),
            Vec3::new(f32::NAN, 0.0, 0.0),
            Vec3::X,
            GuideStyle::default(),
        )
        .is_err()
    );
}

#[test]
fn malformed_style_values_resolve_to_bounded_defaults() {
    let style = GuideStyle {
        width_pixels: f32::NAN,
        opacity: 12.0,
        duty_cycle: -1.0,
        arrow_pixels: f32::INFINITY,
        ..GuideStyle::default()
    };
    let Ok(guide) = Guide::new(owner(), Vec3::ZERO, Vec3::Y * 2.0, style) else {
        panic!("style is sanitized rather than rejected")
    };
    let resolved = guide.style();
    assert!(resolved.width_pixels.is_finite() && resolved.width_pixels > 0.0);
    assert!((0.0..=1.0).contains(&resolved.opacity));
    assert!((0.05..=1.0).contains(&resolved.duty_cycle));
    assert!(resolved.arrow_pixels.is_finite());
}
