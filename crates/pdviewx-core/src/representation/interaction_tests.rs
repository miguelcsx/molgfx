use super::*;
use crate::handle::RawHandle;

fn owner() -> StructureHandle {
    StructureHandle(RawHandle::new_for_test(0, 0))
}

fn edge(kind: InteractionKind) -> InteractionEdge {
    let start = InteractionAnchor::world(Vec3::ZERO).unwrap_or_else(|error| panic!("{error}"));
    let end = InteractionAnchor::world(Vec3::X * 3.0).unwrap_or_else(|error| panic!("{error}"));
    let geometry =
        InteractionGeometry::new(3.0, Some(150.0)).unwrap_or_else(|error| panic!("{error}"));
    InteractionEdge::new(owner(), start, end, kind, geometry, "pdbiox:hbond/v1")
        .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn malformed_interaction_facts_are_typed_errors() {
    let result = InteractionGeometry::new(f32::NAN, None);
    assert_eq!(
        result.err().map(|error| error.code()),
        Some("PDVIEWX-E0035")
    );
    let result = edge(InteractionKind::HydrogenBond).with_occupancy(1.1);
    assert_eq!(
        result.err().map(|error| error.code()),
        Some("PDVIEWX-E0035")
    );
}

#[test]
fn occupancy_and_strength_map_independently_and_reversibly() {
    let edge = edge(InteractionKind::HydrogenBond)
        .with_occupancy(0.5)
        .and_then(|edge| edge.with_normalized_strength(0.4))
        .unwrap_or_else(|error| panic!("{error}"));
    let style = edge.resolved_style();
    assert!((style.opacity - 0.675).abs() < 1.0e-6);
    assert!((style.width_pixels - 2.2).abs() < 1.0e-6);
    assert_eq!(style.pattern, InteractionPattern::Dashes);
}

#[test]
fn phase_speed_is_optional_bounded_presentation_state() {
    let static_style = edge(InteractionKind::HydrogenBond).resolved_style();
    assert!(static_style.phase_speed_pixels_per_frame.abs() < f32::EPSILON);
    let animated = edge(InteractionKind::HydrogenBond)
        .with_phase_speed(2.5)
        .unwrap_or_else(|error| panic!("phase speed validates: {error}"));
    assert!((animated.phase_speed_pixels_per_frame() - 2.5).abs() < f32::EPSILON);
    assert!((animated.resolved_style().phase_speed_pixels_per_frame - 2.5).abs() < f32::EPSILON);
    assert!(
        edge(InteractionKind::HydrogenBond)
            .with_phase_speed(f32::NAN)
            .is_err()
    );
}

#[test]
fn persistence_is_deterministic_presentation_decay_not_source_occupancy() {
    let value = edge(InteractionKind::HydrogenBond)
        .with_occupancy(0.5)
        .and_then(|edge| edge.with_persistence(4, 2.0))
        .unwrap_or_else(|error| panic!("persistence validates: {error}"));
    let style = value.resolved_style();
    let expected = 0.675 * 0.25;
    assert!((style.opacity - expected).abs() < 1.0e-6);
    assert_eq!(value.occupancy(), Some(0.5));
    assert_eq!(value.persistence_age_frames(), 4);
    assert!((value.persistence_half_life_frames() - 2.0).abs() < f32::EPSILON);
    assert!(
        edge(InteractionKind::HydrogenBond)
            .with_persistence(1, f32::NAN)
            .is_err()
    );
}

#[test]
fn every_interaction_class_has_a_distinct_visual_vocabulary() {
    let styles = [
        InteractionKind::HydrogenBond,
        InteractionKind::SaltBridge,
        InteractionKind::PiStacking,
        InteractionKind::Hydrophobic,
        InteractionKind::MetalCoordination,
    ]
    .map(|kind| edge(kind).resolved_style());
    for (index, style) in styles.iter().enumerate() {
        assert!(
            styles[..index]
                .iter()
                .all(|other| other.color != style.color)
        );
    }
}
