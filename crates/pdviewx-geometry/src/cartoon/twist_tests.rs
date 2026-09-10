//! Frame orientation against ring planes.

use super::orient_to_rings;
use pdviewx_math::{CurveSample, TransportFrame, Vec3};

/// Tolerance for a unit axis compared componentwise after normalization.
const EPSILON: f32 = 1.0e-5;

fn sample(tangent: Vec3, segment: u32, parameter: f32) -> CurveSample {
    CurveSample {
        position: Vec3::ZERO,
        tangent,
        segment,
        parameter,
    }
}

/// A twist-free frame carrying the given tangent, as parallel transport would
/// hand one over before any ring plane is consulted.
fn transported(tangent: Vec3) -> TransportFrame {
    let normal = tangent.any_orthonormal_vector();
    TransportFrame {
        tangent,
        normal,
        binormal: tangent.cross(normal),
    }
}

fn assert_axis(actual: Vec3, expected: Vec3, what: &str) {
    assert!(
        (actual - expected).length() < EPSILON,
        "{what}: expected {expected:?}, got {actual:?}"
    );
}

#[test]
fn a_ring_plane_across_the_curve_becomes_the_ribbon_thickness_axis() {
    let samples = [sample(Vec3::X, 0, 0.0)];
    let mut frames = [transported(Vec3::X)];
    orient_to_rings(&samples, &[Vec3::Z, Vec3::Z], &mut frames);
    let [frame] = frames;
    assert_axis(frame.binormal, Vec3::Z, "thickness axis");
    // The house convention is binormal == tangent x normal, so the width axis
    // is what closes that identity rather than an independent choice.
    assert_axis(frame.normal, Vec3::Y, "width axis");
    assert_axis(
        frame.tangent.cross(frame.normal),
        frame.binormal,
        "handedness",
    );
}

#[test]
fn the_ribbon_turns_halfway_between_two_rings_a_quarter_turn_apart() {
    let samples = [sample(Vec3::X, 0, 0.5)];
    let mut frames = [transported(Vec3::X)];
    orient_to_rings(&samples, &[Vec3::Z, Vec3::Y], &mut frames);
    let [frame] = frames;
    let halfway = (Vec3::Z + Vec3::Y).normalize();
    assert_axis(frame.binormal, halfway, "thickness axis at the midpoint");
}

#[test]
fn a_ring_plane_running_along_the_curve_leaves_the_transport_frame_alone() {
    let samples = [sample(Vec3::X, 0, 0.0)];
    let original = transported(Vec3::X);
    let mut frames = [original];
    // A normal parallel to the tangent determines no face, so there is nothing
    // to orient to and the twist-free frame has to survive untouched.
    orient_to_rings(&samples, &[Vec3::X, Vec3::X], &mut frames);
    let [frame] = frames;
    assert_axis(frame.normal, original.normal, "width axis");
    assert_axis(frame.binormal, original.binormal, "thickness axis");
}

#[test]
fn a_trace_without_ring_planes_keeps_every_transport_frame() {
    let samples = [sample(Vec3::X, 0, 0.0), sample(Vec3::Y, 1, 0.0)];
    let original = [transported(Vec3::X), transported(Vec3::Y)];
    let mut frames = original;
    orient_to_rings(&samples, &[], &mut frames);
    assert_eq!(frames, original);
}

#[test]
fn a_sample_past_the_last_ring_plane_holds_the_final_orientation() {
    // The last control point opens no interval, so a sample landing on it has
    // no successor to blend towards and must reuse the plane it sits in.
    let samples = [sample(Vec3::X, 1, 0.75)];
    let mut frames = [transported(Vec3::X)];
    orient_to_rings(&samples, &[Vec3::Y, Vec3::Z], &mut frames);
    let [frame] = frames;
    assert_axis(
        frame.binormal,
        Vec3::Z,
        "thickness axis at the trailing end",
    );
}

#[test]
fn a_plane_blend_that_would_dive_into_the_tangent_still_turns_evenly() {
    // Two ring planes whose perpendicular parts point almost opposite ways. A
    // blend of the raw normals passes close to the tangent partway across the
    // interval, where the flat face is undetermined and the old frame snapped
    // through a right angle in one step. Projecting first turns the blend into a
    // rotation about the tangent, which has no such point.
    let start = Vec3::new(0.6, 0.0, 0.8).normalize();
    let end = Vec3::new(0.6, 0.1, -0.79).normalize();
    let steps = 24u8;
    let mut previous: Option<Vec3> = None;
    for step in 0..=steps {
        let parameter = f32::from(step) / f32::from(steps);
        let samples = [sample(Vec3::X, 0, parameter)];
        let mut frames = [transported(Vec3::X)];
        orient_to_rings(&samples, &[start, end], &mut frames);
        let [frame] = frames;
        if let Some(previous) = previous {
            let turn = previous
                .dot(frame.binormal)
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees();
            assert!(turn < 20.0, "frame jumped {turn} degrees in one sample");
        }
        previous = Some(frame.binormal);
    }
}
