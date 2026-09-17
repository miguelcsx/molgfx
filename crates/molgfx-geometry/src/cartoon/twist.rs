//! Ring-anchored frames: the twist in the glycan twister ribbon.
//!
//! A backbone ribbon is framed by parallel transport, which is deliberately
//! twist-free — a protein's guide atoms are points on a line and carry no plane
//! of their own, so any twist the ribbon showed would be invented.
//!
//! A sugar does carry a plane. Its ring is the body of the residue, and the
//! angle between one ring and the next is the glycosidic geometry itself. So
//! the twister ribbon is framed by those planes instead: the flat face is held
//! in each ring's plane, and the twist the viewer reads between two sugars is
//! the rotation the molecule actually makes, not a rendering artefact.
//!
//! Costs `O(samples)` and allocates nothing; frames are rewritten in place.

use molgfx_math::{CurveSample, TransportFrame, Vec3};

#[cfg(test)]
#[path = "twist_tests.rs"]
mod tests;

/// Samples per radian of twist, as an extra demand for the adaptive sampler.
///
/// Seven degrees per quad is about where a flat face stops reading as a fold,
/// and a radian is a little over eight of those.
const SAMPLES_PER_RADIAN: f32 = 8.2;

/// Shortest projection of a ring normal onto the plane perpendicular to the
/// tangent that is still a direction rather than rounding error.
const MINIMUM_PROJECTION_SQ: f32 = 1.0e-8;

/// Re-frames sampled points so the ribbon's flat face lies in the ring plane
/// interpolated across each control-point interval.
///
/// Samples whose interval has no usable plane — a collapsed ring, or a ring
/// whose normal runs along the curve so that no flat face is determined — keep
/// the parallel-transport frame they arrived with, so an unorientable sugar
/// degrades to the twist-free ribbon rather than to a degenerate one.
pub(super) fn orient_to_rings(
    samples: &[CurveSample],
    normals: &[Vec3],
    frames: &mut [TransportFrame],
) {
    if normals.is_empty() {
        return;
    }
    for (sample, frame) in samples.iter().zip(frames.iter_mut()) {
        let Some(oriented) = orient(sample, frame.tangent, normals) else {
            continue;
        };
        *frame = oriented;
    }
}

/// Builds the frame whose thickness axis is the blended ring plane.
///
/// Both ring normals are taken into the cross-section plane — the plane
/// perpendicular to the tangent — *before* they are blended, not after. Blending
/// the raw normals and projecting the result lets that result swing towards the
/// tangent direction partway along an interval: the ring plane then says nothing
/// about which way the flat face looks, the surviving perpendicular component is
/// mostly rounding error, and the frame sweeps through most of a right angle in
/// a single sample. On the ribbon that draws as a hard crease, which a reader
/// takes for a fold in the molecule.
///
/// Once both planes are in that one plane, the turn between them is an angle
/// about the tangent, so the blend is a rotation by that angle rather than a
/// lerp of two directions. A lerp crowds most of a wide turn into the middle of
/// the interval, which is exactly where a ribbon facets; rotating spreads it
/// evenly, and it stays defined even for a half turn, where a lerp passes
/// through zero.
///
/// The rotation is eased rather than linear: a constant rate is only C0 across a
/// control point, so the twist rate jumps at every ring. Easing brings the rate
/// to zero at both ends of each interval.
///
/// The width axis then follows from the house convention that the thickness axis
/// is the tangent crossed into the width axis.
fn orient(sample: &CurveSample, tangent: Vec3, normals: &[Vec3]) -> Option<TransportFrame> {
    let segment = usize::try_from(sample.segment).ok()?;
    let start = flatten(normals.get(segment).copied()?, tangent)?;
    let end = match normals.get(segment + 1).copied() {
        Some(value) => match flatten(value, tangent) {
            Some(flattened) => flattened,
            None => start,
        },
        None => start,
    };
    let across = tangent.cross(start);
    let angle = f32::atan2(end.dot(across), end.dot(start));
    let amount = sample.parameter.clamp(0.0, 1.0);
    let turned = angle * (amount * amount * (3.0 - 2.0 * amount));
    let binormal = (start * turned.cos() + across * turned.sin()).try_normalize()?;
    let normal = binormal.cross(tangent).try_normalize()?;
    Some(TransportFrame {
        tangent,
        normal,
        binormal,
    })
}

/// One ring plane taken into the cross-section plane at this sample.
///
/// A ring whose normal runs along the curve determines no flat face at all, and
/// what is left of it after projection is rounding error, so it reports nothing
/// rather than a direction.
fn flatten(plane: Vec3, tangent: Vec3) -> Option<Vec3> {
    let projected = plane - tangent * plane.dot(tangent);
    if projected.length_squared() < MINIMUM_PROJECTION_SQ {
        return None;
    }
    projected.try_normalize()
}

/// Extra per-interval sample demand from how far the ribbon has to turn.
///
/// The adaptive sampler measures midpoint deviation and tangent turn — the
/// shape of the curve — and a glycan is nearly straight between ring centres
/// while its planes rotate through most of a half turn. Left to the shape
/// alone the trace earns one or two samples per sugar and has to do that
/// rotation inside a single quad, which folds. Reporting the rotation buys
/// density where the ribbon actually twists instead of along its whole length,
/// which is the difference between a uniformly dense trace and one that costs
/// what it needs.
///
/// The turn is measured against the chord rather than the true tangent, which
/// is not known yet: the samples it comes from are what this decides. Over one
/// glycosidic link the two differ by little, and the sampler rounds the answer
/// up to a power of two regardless.
pub(super) fn twist_demand(trace: &[Vec3], normals: &[Vec3], out: &mut Vec<f32>) {
    out.clear();
    if normals.len() < 2 || trace.len() < 2 {
        return;
    }
    out.reserve(trace.len() - 1);
    for (index, window) in trace.windows(2).enumerate() {
        out.push(chord_turn(
            window,
            normals.get(index),
            normals.get(index + 1),
        ));
    }
}

/// Rotation about one interval's chord between two ring planes, in radians.
fn chord_turn(window: &[Vec3], start: Option<&Vec3>, end: Option<&Vec3>) -> f32 {
    let (Some(&first), Some(&second)) = (window.first(), window.get(1)) else {
        return 0.0;
    };
    let Some(chord) = (second - first).try_normalize() else {
        return 0.0;
    };
    let (Some(&start), Some(&end)) = (start, end) else {
        return 0.0;
    };
    let (Some(start), Some(end)) = (flatten(start, chord), flatten(end, chord)) else {
        return 0.0;
    };
    let across = chord.cross(start);
    f32::atan2(end.dot(across), end.dot(start)).abs() * SAMPLES_PER_RADIAN
}
