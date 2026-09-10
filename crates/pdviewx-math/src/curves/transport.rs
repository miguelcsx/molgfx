//! Twist-free parallel-transport frames along a sampled curve.

use crate::{CurveSample, Quat, Vec3};

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;

/// Orthonormal frame carried along a curve.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TransportFrame {
    /// Curve direction.
    pub tangent: Vec3,
    /// Ribbon-width axis.
    pub normal: Vec3,
    /// Ribbon-thickness axis.
    pub binormal: Vec3,
}

/// Builds minimal-rotation frames into caller-owned storage. The initial axis
/// is selected deterministically to avoid degeneracy near a world axis.
pub fn parallel_transport(samples: &[CurveSample], out: &mut Vec<TransportFrame>) {
    out.clear();
    let Some(first) = samples.first() else {
        return;
    };
    let tangent = normalized(first.tangent, Vec3::Z);
    let seed = least_aligned_axis(tangent);
    let normal = normalized(seed.cross(tangent), Vec3::X);
    out.push(frame(tangent, normal));
    for sample in &samples[1..] {
        let Some(previous) = out.last().copied() else {
            break;
        };
        let next_tangent = normalized(sample.tangent, previous.tangent);
        let rotation = Quat::from_rotation_arc(previous.tangent, next_tangent);
        // The rotation carries the previous normal to one perpendicular to the
        // new tangent. Renormalizing that single vector each step bounds
        // floating-point drift; the binormal is then an exact unit cross of two
        // orthonormal vectors, so no further Gram-Schmidt pass is needed.
        let normal = normalized(rotation * previous.normal, previous.normal);
        out.push(TransportFrame {
            tangent: next_tangent,
            normal,
            binormal: next_tangent.cross(normal),
        });
    }
}

#[inline]
fn frame(tangent: Vec3, normal_hint: Vec3) -> TransportFrame {
    let normal = normalized(normal_hint - tangent * normal_hint.dot(tangent), Vec3::X);
    let binormal = normalized(tangent.cross(normal), Vec3::Y);
    TransportFrame {
        tangent,
        normal: normalized(binormal.cross(tangent), normal),
        binormal,
    }
}

#[inline]
fn normalized(value: Vec3, fallback: Vec3) -> Vec3 {
    match value.try_normalize() {
        Some(unit) => unit,
        None => fallback,
    }
}

#[inline]
fn least_aligned_axis(tangent: Vec3) -> Vec3 {
    let absolute = tangent.abs();
    if absolute.x <= absolute.y && absolute.x <= absolute.z {
        Vec3::X
    } else if absolute.y <= absolute.z {
        Vec3::Y
    } else {
        Vec3::Z
    }
}
