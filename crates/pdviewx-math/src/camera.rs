//! The camera: an eye, a target, an up hint and a projection.
//!
//! The full model→world→view→clip chain is composed here in one fixed order
//! with fixed associativity, so an identical camera and scene produce
//! bit-identical clip coordinates run after run.

use crate::aabb::BoundingSphere;
use crate::projection::Projection;
use glam::{Mat4, Vec3, Vec4};

#[cfg(test)]
#[path = "camera_tests.rs"]
mod tests;

/// A look-at camera in world space (right-handed, +Y up, looking down −Z in
/// its own view space).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Camera {
    /// Eye position, world space.
    pub eye: Vec3,
    /// Point the camera looks at, world space.
    pub target: Vec3,
    /// Up hint; need not be orthogonal to the view direction.
    pub up: Vec3,
    /// The view-to-clip mapping.
    pub projection: Projection,
}

impl Camera {
    /// A camera looking at `bound` from a distance that frames it fully,
    /// down the +Z axis with +Y up.
    #[must_use]
    pub fn framing(bound: &BoundingSphere, aspect: f32) -> Self {
        let fov_y = std::f32::consts::FRAC_PI_4;
        let radius = bound.radius.max(1.0);
        // Fit the sphere in both the vertical and horizontal fields of view.
        let half_min_fov = if aspect < 1.0 {
            (fov_y * 0.5).tan() * aspect
        } else {
            (fov_y * 0.5).tan()
        };
        let distance = radius / half_min_fov.clamp(1e-3, 1.0) * 1.2;
        let eye = bound.center + Vec3::new(0.0, 0.0, distance);
        let mut projection = Projection::Perspective {
            fov_y,
            aspect,
            near: 0.1,
            far: distance + radius * 2.0,
        };
        projection.fit_near_far(eye, bound);
        Self {
            eye,
            target: bound.center,
            up: Vec3::Y,
            projection,
        }
    }

    /// The view-from-world matrix.
    #[must_use]
    pub fn view(&self) -> Mat4 {
        glam::camera::rh::view::look_at_mat4(self.eye, self.target, self.up)
    }

    /// The clip-from-world matrix: projection composed with view, in that
    /// order, always.
    #[must_use]
    pub fn view_proj(&self) -> Mat4 {
        self.projection.matrix() * self.view()
    }

    /// The six frustum planes of `view_proj`, as `(normal, d)` packed into
    /// `Vec4` with the inside satisfying `dot(n, p) + d >= 0`. Order: left,
    /// right, bottom, top, near, far.
    #[must_use]
    pub fn frustum_planes(&self) -> [Vec4; 6] {
        let m = self.view_proj();
        let row = |i: usize| m.row(i);
        let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
        let normalize = |p: Vec4| {
            let len = p.truncate().length();
            if len > 0.0 { p / len } else { p }
        };
        [
            normalize(r3 + r0), // left
            normalize(r3 - r0), // right
            normalize(r3 + r1), // bottom
            normalize(r3 - r1), // top
            // Reversed depth: clip z spans [0, w] with near at w and far at 0,
            // so z >= 0 is the far side and w - z >= 0 the near side.
            normalize(r3 - r2), // near
            normalize(r2),      // far
        ]
    }

    /// Distance from eye to target.
    #[must_use]
    pub fn focus_distance(&self) -> f32 {
        self.eye.distance(self.target)
    }
}
