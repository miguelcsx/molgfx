//! Axis-aligned bounds and bounding spheres.
//!
//! Building a bound over `n` points is `O(n)`; every query on a built bound
//! is `O(1)`.

use glam::{Mat4, Vec3};

#[cfg(test)]
#[path = "aabb_tests.rs"]
mod tests;

/// An axis-aligned bounding box in whichever space its points came from.
///
/// The empty box has `min > max` on every axis, so extending it with the
/// first point produces a degenerate box at that point, and a union with any
/// box returns the other operand.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Aabb {
    /// Componentwise minimum corner.
    pub min: Vec3,
    /// Componentwise maximum corner.
    pub max: Vec3,
}

impl Default for Aabb {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl Aabb {
    /// The identity of `union`: contains nothing, extends from any point.
    pub const EMPTY: Self = Self {
        min: Vec3::splat(f32::INFINITY),
        max: Vec3::splat(f32::NEG_INFINITY),
    };

    /// Builds a box from explicit corners; callers guarantee `min <= max`.
    #[must_use]
    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Builds the tightest box over a set of points, `O(n)`.
    /// Non-finite coordinates are skipped so one bad atom cannot poison the
    /// bound of a whole structure.
    #[must_use]
    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Self {
        let mut aabb = Self::EMPTY;
        for p in points {
            aabb.extend(p);
        }
        aabb
    }

    /// True when no point has been added.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }

    /// Grows the box to contain `point`; non-finite points are ignored.
    pub fn extend(&mut self, point: Vec3) {
        if point.is_finite() {
            self.min = self.min.min(point);
            self.max = self.max.max(point);
        }
    }

    /// Grows the box to contain a sphere of `radius` around `center`.
    pub fn extend_sphere(&mut self, center: Vec3, radius: f32) {
        if center.is_finite() && radius.is_finite() {
            let r = Vec3::splat(radius.abs());
            self.min = self.min.min(center - r);
            self.max = self.max.max(center + r);
        }
    }

    /// The smallest box containing both operands.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// Center point; meaningless on an empty box.
    #[must_use]
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Half the diagonal extent on each axis.
    #[must_use]
    pub fn half_extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    /// The eight corner points, fixed order (x fastest, then y, then z).
    #[must_use]
    pub fn corners(&self) -> [Vec3; 8] {
        let (lo, hi) = (self.min, self.max);
        [
            Vec3::new(lo.x, lo.y, lo.z),
            Vec3::new(hi.x, lo.y, lo.z),
            Vec3::new(lo.x, hi.y, lo.z),
            Vec3::new(hi.x, hi.y, lo.z),
            Vec3::new(lo.x, lo.y, hi.z),
            Vec3::new(hi.x, lo.y, hi.z),
            Vec3::new(lo.x, hi.y, hi.z),
            Vec3::new(hi.x, hi.y, hi.z),
        ]
    }

    /// The tightest axis-aligned box containing this box after an affine
    /// transform: the transformed corners' bound.
    #[must_use]
    pub fn transform(&self, matrix: &Mat4) -> Self {
        if self.is_empty() {
            return Self::EMPTY;
        }
        let mut out = Self::EMPTY;
        for corner in self.corners() {
            out.extend(matrix.transform_point3(corner));
        }
        out
    }

    /// Slab test: the parametric interval where a ray overlaps the box, or
    /// `None` when it misses. `inv_dir` is the componentwise reciprocal of
    /// the ray direction (infinities from zero components behave correctly).
    #[must_use]
    pub fn ray_intersect(&self, origin: Vec3, inv_dir: Vec3) -> Option<(f32, f32)> {
        let t0 = (self.min - origin) * inv_dir;
        let t1 = (self.max - origin) * inv_dir;
        let t_near = t0.min(t1).max_element();
        let t_far = t0.max(t1).min_element();
        if t_near <= t_far && t_far >= 0.0 {
            Some((t_near.max(0.0), t_far))
        } else {
            None
        }
    }

    /// The bounding sphere of the box: centered, radius to a corner.
    #[must_use]
    pub fn bounding_sphere(&self) -> BoundingSphere {
        if self.is_empty() {
            return BoundingSphere {
                center: Vec3::ZERO,
                radius: 0.0,
            };
        }
        BoundingSphere {
            center: self.center(),
            radius: self.half_extents().length(),
        }
    }
}

/// A sphere enclosing a set of geometry; the shape cameras frame against.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BoundingSphere {
    /// Sphere center.
    pub center: Vec3,
    /// Sphere radius; never negative.
    pub radius: f32,
}

impl BoundingSphere {
    /// Encloses a set of points in two `O(n)` passes: centroid, then the
    /// farthest distance from it. Not minimal, but tight enough for camera
    /// framing and never smaller than the points it covers. The centroid
    /// accumulates in f64 so summing a million single-precision coordinates
    /// does not drift; only the result returns to f32.
    #[must_use]
    pub fn from_points(points: &[Vec3]) -> Self {
        let mut sum = glam::DVec3::ZERO;
        let mut count = 0u32;
        for p in points {
            if p.is_finite() {
                sum += p.as_dvec3();
                count += 1;
            }
        }
        if count == 0 {
            return Self {
                center: Vec3::ZERO,
                radius: 0.0,
            };
        }
        let center = (sum / f64::from(count)).as_vec3();
        let mut radius_sq = 0.0f32;
        for p in points {
            if p.is_finite() {
                radius_sq = radius_sq.max(center.distance_squared(*p));
            }
        }
        Self {
            center,
            radius: radius_sq.sqrt(),
        }
    }
}
