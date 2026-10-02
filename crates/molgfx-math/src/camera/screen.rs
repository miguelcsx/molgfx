//! The mapping between world points and screen pixels.
//!
//! A host that draws its own overlay on top of a rendered image — a callout
//! anchored to an atom, a legend beside a residue, a hit test on a custom
//! shape — needs the same mapping the renderer applies. These are that mapping,
//! with the renderer's conventions: pixels from the top-left, and the camera's
//! aspect set to the target's.

use super::Camera;
use crate::{Vec3, Vec4};
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "screen_tests.rs"]
mod tests;

/// A world point as it lands on a render target.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct ScreenPoint {
    /// Pixels from the left edge; outside `0..width` when the point is off-screen.
    pub x: f32,
    /// Pixels from the top edge; outside `0..height` when the point is off-screen.
    pub y: f32,
    /// Distance from the eye along the view direction, in Ångström.
    pub depth: f32,
}

/// A half-line from the eye, or from the image plane for an orthographic camera.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Ray {
    /// Where the ray starts, world space.
    pub origin: Vec3,
    /// Unit direction, world space.
    pub direction: Vec3,
}

impl Camera {
    /// This camera with its aspect ratio set to a target's, as the renderer does.
    fn for_target(&self, size: (u32, u32)) -> Option<(Self, f32, f32)> {
        let width = f32::from(u16::try_from(size.0).ok()?).max(1.0);
        let height = f32::from(u16::try_from(size.1).ok()?).max(1.0);
        let mut camera = *self;
        camera.projection.set_aspect(width / height);
        Some((camera, width, height))
    }

    /// Where a world point lands on a `size` render target, in pixels from the
    /// top-left. `None` when the point is behind the eye or not finite.
    #[must_use]
    pub fn project(&self, point: Vec3, size: (u32, u32)) -> Option<ScreenPoint> {
        let (camera, width, height) = self.for_target(size)?;
        if !point.is_finite() {
            return None;
        }
        let eye_space = camera.view().transform_point3(point);
        let depth = -eye_space.z;
        if depth.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return None;
        }
        let clip =
            camera.projection.matrix() * Vec4::new(eye_space.x, eye_space.y, eye_space.z, 1.0);
        if clip.w.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return None;
        }
        Some(ScreenPoint {
            x: (clip.x / clip.w * 0.5 + 0.5) * width,
            y: (0.5 - clip.y / clip.w * 0.5) * height,
            depth,
        })
    }

    /// The ray through the pixel `(x, y)` of a `size` render target, measured
    /// from the top-left. `None` for a degenerate camera or target.
    #[must_use]
    pub fn ray(&self, x: f32, y: f32, size: (u32, u32)) -> Option<Ray> {
        let (camera, width, height) = self.for_target(size)?;
        let ndc_x = x / width * 2.0 - 1.0;
        let ndc_y = 1.0 - y / height * 2.0;
        let forward = (camera.target - camera.eye).try_normalize()?;
        let right = forward.cross(camera.up).try_normalize()?;
        let up = right.cross(forward);
        let (origin, direction) = match camera.projection {
            crate::Projection::Perspective { fov_y, aspect, .. } => {
                let half_height = (fov_y * 0.5).tan();
                let direction =
                    forward + right * (ndc_x * half_height * aspect) + up * (ndc_y * half_height);
                (camera.eye, direction.try_normalize()?)
            }
            crate::Projection::Orthographic { height, aspect, .. } => {
                let half_height = height * 0.5;
                (
                    camera.eye
                        + right * (ndc_x * half_height * aspect)
                        + up * (ndc_y * half_height),
                    forward,
                )
            }
        };
        Some(Ray { origin, direction })
    }
}
