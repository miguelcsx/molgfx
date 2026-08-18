//! Cinematic secondary motion over a ribbon spine.
//!
//! This is art direction, not molecular dynamics. It exists so an explanatory
//! animation can give a ribbon the trailing follow-through a real filament has,
//! and it says so: the solver never touches source coordinates, it only produces
//! an offset the presentation layer adds. Picking, measurement, interaction
//! detection and provenance keep resolving against the coordinates the file
//! supplied, so nothing here can be mistaken for simulated motion.
//!
//! The displacement is a smooth deterministic field along arc length, relaxed by
//! a fixed number of position-based iterations that keep segment lengths and
//! curvature plausible. Fixed timestep, fixed iteration count and an integer
//! hash for the field mean the same spine and phase resolve to the same offsets
//! on every frame, machine and run.

use pdviewx_math::Vec3;

#[cfg(test)]
#[path = "secondary_motion_tests.rs"]
mod tests;

/// Caller-authored secondary-motion settings. Every field is presentation
/// state; none of it is derived from the structure.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SecondaryMotion {
    /// Peak offset in Ångström, before constraints pull it back.
    pub amplitude: f32,
    /// Waves along the spine; higher values ripple more tightly.
    pub wavelength_samples: f32,
    /// Animation phase in turns. Callers advance it per frame.
    pub phase: f32,
    /// Constraint iterations. Fixed, so the result never depends on framerate.
    pub iterations: u8,
    /// How strongly segment lengths are preserved, in `[0, 1]`.
    pub stiffness: f32,
    /// Fraction of the spine at each end pinned to the source, in `[0, 0.5]`.
    pub anchor_fraction: f32,
}

impl Default for SecondaryMotion {
    fn default() -> Self {
        Self {
            amplitude: 0.0,
            wavelength_samples: 24.0,
            phase: 0.0,
            iterations: 8,
            stiffness: 0.6,
            anchor_fraction: 0.12,
        }
    }
}

impl SecondaryMotion {
    /// A restrained follow-through suitable for explanatory animation.
    #[must_use]
    pub const fn gentle() -> Self {
        Self {
            amplitude: 0.9,
            wavelength_samples: 28.0,
            phase: 0.0,
            iterations: 8,
            stiffness: 0.6,
            anchor_fraction: 0.12,
        }
    }

    /// Finite, bounded settings consumed by the solver.
    #[must_use]
    pub fn sanitized(self) -> Self {
        let default = Self::default();
        Self {
            amplitude: bounded(self.amplitude, 0.0, 32.0, default.amplitude),
            wavelength_samples: bounded(self.wavelength_samples, 2.0, 512.0, 24.0),
            phase: if self.phase.is_finite() {
                self.phase
            } else {
                0.0
            },
            iterations: self.iterations.clamp(1, 32),
            stiffness: bounded(self.stiffness, 0.0, 1.0, default.stiffness),
            anchor_fraction: bounded(self.anchor_fraction, 0.0, 0.5, default.anchor_fraction),
        }
    }

    /// True when the layer contributes nothing and can be skipped entirely.
    #[must_use]
    pub fn is_neutral(self) -> bool {
        !(self.amplitude.is_finite() && self.amplitude > 0.0)
    }
}

/// Solves the offset each spine sample receives.
///
/// `spine` is the scientific geometry and is never modified. `offsets` is
/// cleared and filled with one displacement per sample, so the caller can add
/// it into a separate presentation buffer and drop it without rebuilding the
/// scene.
pub fn solve_offsets(spine: &[Vec3], motion: SecondaryMotion, offsets: &mut Vec<Vec3>) {
    offsets.clear();
    let motion = motion.sanitized();
    if motion.is_neutral() || spine.len() < 3 {
        offsets.resize(spine.len(), Vec3::ZERO);
        return;
    }

    // Start from a smooth transverse field: a filament trails across its own
    // direction, never along it, so displacing along the tangent would only
    // stretch the ribbon.
    let count = spine.len();
    let mut displaced = Vec::with_capacity(count);
    let mut index_position = 0.0f32;
    for (index, point) in spine.iter().enumerate() {
        let tangent = spine_tangent(spine, index);
        let side = transverse_axis(tangent);
        let lift = tangent.cross(side);
        let turns = index_position / motion.wavelength_samples + motion.phase;
        let angle = turns * std::f32::consts::TAU;
        let taper = anchor_taper(index, count, motion.anchor_fraction);
        let swing = motion.amplitude * taper;
        displaced.push(*point + (side * angle.sin() + lift * angle.cos() * 0.45) * swing);
        index_position += 1.0;
    }

    // Relax: keep the original segment lengths and resist sharp kinks, with the
    // anchored ends held to the source spine.
    for _ in 0..motion.iterations {
        relax_lengths(spine, &mut displaced, motion);
        relax_bending(&mut displaced, motion);
        pin_anchors(spine, &mut displaced, motion.anchor_fraction);
    }

    offsets.extend(
        displaced
            .iter()
            .zip(spine)
            .map(|(moved, source)| *moved - *source),
    );
}

fn spine_tangent(spine: &[Vec3], index: usize) -> Vec3 {
    let previous = spine.get(index.saturating_sub(1)).copied();
    let next = spine.get((index + 1).min(spine.len() - 1)).copied();
    let direction = match (previous, next) {
        (Some(previous), Some(next)) => next - previous,
        _ => Vec3::X,
    };
    direction.try_normalize().map_or(Vec3::X, |unit| unit)
}

/// A stable axis perpendicular to the tangent, chosen away from the world axis
/// the tangent is most aligned with so the cross product never degenerates.
fn transverse_axis(tangent: Vec3) -> Vec3 {
    let absolute = tangent.abs();
    let seed = if absolute.x <= absolute.y && absolute.x <= absolute.z {
        Vec3::X
    } else if absolute.y <= absolute.z {
        Vec3::Y
    } else {
        Vec3::Z
    };
    seed.cross(tangent)
        .try_normalize()
        .map_or(Vec3::Y, |unit| unit)
}

/// Ends fade to zero so the deformation never detaches the ribbon from the
/// coordinates it belongs to.
fn anchor_taper(index: usize, count: usize, anchor_fraction: f32) -> f32 {
    if count < 2 {
        return 0.0;
    }
    let last = count - 1;
    let from_start = index;
    let from_end = last - index;
    let nearest = from_start.min(from_end);
    let anchored = (f32::from(u16::try_from(count).map_or(u16::MAX, |value| value))
        * anchor_fraction)
        .max(1.0);
    let distance = f32::from(u16::try_from(nearest).map_or(u16::MAX, |value| value));
    (distance / anchored).clamp(0.0, 1.0)
}

fn relax_lengths(spine: &[Vec3], displaced: &mut [Vec3], motion: SecondaryMotion) {
    for index in 1..displaced.len() {
        let (Some(source_previous), Some(source_current)) =
            (spine.get(index - 1), spine.get(index))
        else {
            continue;
        };
        let rest = source_previous.distance(*source_current);
        let (Some(previous), Some(current)) = (
            displaced.get(index - 1).copied(),
            displaced.get(index).copied(),
        ) else {
            continue;
        };
        let delta = current - previous;
        let length = delta.length();
        if length <= f32::EPSILON {
            continue;
        }
        let correction = delta * ((length - rest) / length) * 0.5 * motion.stiffness;
        if let Some(slot) = displaced.get_mut(index - 1) {
            *slot = previous + correction;
        }
        if let Some(slot) = displaced.get_mut(index) {
            *slot = current - correction;
        }
    }
}

/// Pull each interior sample toward the midpoint of its neighbours, which
/// removes the kinks length correction alone can leave behind.
fn relax_bending(displaced: &mut [Vec3], motion: SecondaryMotion) {
    let smoothing = 0.5 * (1.0 - motion.stiffness).clamp(0.0, 1.0);
    if smoothing <= 0.0 || displaced.len() < 3 {
        return;
    }
    for index in 1..displaced.len() - 1 {
        let (Some(previous), Some(current), Some(next)) = (
            displaced.get(index - 1).copied(),
            displaced.get(index).copied(),
            displaced.get(index + 1).copied(),
        ) else {
            continue;
        };
        let target = (previous + next) * 0.5;
        if let Some(slot) = displaced.get_mut(index) {
            *slot = current + (target - current) * smoothing;
        }
    }
}

fn pin_anchors(spine: &[Vec3], displaced: &mut [Vec3], anchor_fraction: f32) {
    let count = displaced.len();
    for index in 0..count {
        if anchor_taper(index, count, anchor_fraction) > 0.0 {
            continue;
        }
        let (Some(source), Some(slot)) = (spine.get(index), displaced.get_mut(index)) else {
            continue;
        };
        *slot = *source;
    }
}

fn bounded(value: f32, minimum: f32, maximum: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(minimum, maximum)
    } else {
        fallback
    }
}
