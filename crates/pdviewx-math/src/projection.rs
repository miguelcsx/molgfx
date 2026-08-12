//! Projection construction with reversed [0, 1] clip depth.
//!
//! Reversed depth (near plane at 1.0, far at 0.0) is applied here, once, at
//! matrix construction; nothing downstream reasons about the clip convention
//! again. Near and far are fit to the scene's bounding sphere rather than
//! fixed, so depth precision follows the structure on screen.

use crate::aabb::BoundingSphere;
use glam::{Mat4, Vec3};

#[cfg(test)]
#[path = "projection_tests.rs"]
mod tests;

/// The closest the fitted near plane may come to the camera, in Ångström.
/// Keeps the projection finite when the camera sits inside the scene bound.
const MIN_NEAR: f32 = 0.01;

/// How a camera maps view space to clip space.
///
/// Orthographic projection is offered as a first-class choice because it is
/// often the scientifically correct one: parallel lines stay parallel and
/// distances compare across the image.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Projection {
    /// Perspective projection with a vertical field of view in radians.
    Perspective {
        /// Vertical field of view, radians.
        fov_y: f32,
        /// Width over height.
        aspect: f32,
        /// Near plane distance, Ångström; maps to clip depth 1.0.
        near: f32,
        /// Far plane distance, Ångström; maps to clip depth 0.0.
        far: f32,
    },
    /// Orthographic projection with an explicit vertical extent.
    Orthographic {
        /// Full visible height, Ångström.
        height: f32,
        /// Width over height.
        aspect: f32,
        /// Near plane distance, Ångström; maps to clip depth 1.0.
        near: f32,
        /// Far plane distance, Ångström; maps to clip depth 0.0.
        far: f32,
    },
}

impl Projection {
    /// The clip-from-view matrix with reversed [0, 1] depth.
    #[must_use]
    pub fn matrix(&self) -> Mat4 {
        match *self {
            Self::Perspective {
                fov_y,
                aspect,
                near,
                far,
            } => {
                // The right-handed [0, 1] projection maps near→0, far→1;
                // swapping the plane arguments reverses the depth range.
                glam::camera::rh::proj::directx::perspective(fov_y, aspect, far, near)
            }
            Self::Orthographic {
                height,
                aspect,
                near,
                far,
            } => {
                let half_h = height * 0.5;
                let half_w = half_h * aspect;
                glam::camera::rh::proj::directx::orthographic(
                    -half_w, half_w, -half_h, half_h, far, near,
                )
            }
        }
    }

    /// Refits the near and far planes to a world-space bounding sphere seen
    /// from `eye` looking toward the sphere. Called on camera change so the
    /// depth range always brackets the scene tightly.
    pub fn fit_near_far(&mut self, eye: Vec3, bound: &BoundingSphere) {
        let distance = eye.distance(bound.center);
        let pad = bound.radius.max(1.0) * 0.01;
        let new_far = distance + bound.radius + pad;
        let new_near = (distance - bound.radius - pad).max(MIN_NEAR);
        match self {
            Self::Perspective { near, far, .. } | Self::Orthographic { near, far, .. } => {
                *near = new_near;
                *far = new_far;
            }
        }
    }

    /// Width over height.
    #[must_use]
    pub fn aspect(&self) -> f32 {
        match *self {
            Self::Perspective { aspect, .. } | Self::Orthographic { aspect, .. } => aspect,
        }
    }

    /// Updates the aspect ratio, preserving everything else.
    pub fn set_aspect(&mut self, new_aspect: f32) {
        match self {
            Self::Perspective { aspect, .. } | Self::Orthographic { aspect, .. } => {
                *aspect = new_aspect;
            }
        }
    }
}
