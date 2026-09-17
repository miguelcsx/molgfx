//! Event-driven camera paths and restorable spatiotemporal bookmarks.

use crate::{CoreError, Scene, Timeline};
use molgfx_math::{Camera, Mat3, Projection, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[cfg(test)]
#[path = "camera_path_tests.rs"]
mod tests;

/// Interpolation timing applied inside every camera-path segment.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CameraEasing {
    /// Constant motion through the segment.
    Linear,
    /// Zero velocity at both segment endpoints.
    #[default]
    SmoothStep,
}

/// One validated camera state at a finite local timestamp.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CameraKeyframe {
    time_seconds: f64,
    camera: Camera,
}

impl CameraKeyframe {
    /// Creates one finite, non-degenerate camera keyframe.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTimeline`] for a malformed time, view basis
    /// or projection.
    pub fn new(time_seconds: f64, camera: Camera) -> Result<Self, CoreError> {
        validate_camera(camera)?;
        if !time_seconds.is_finite() {
            return Err(invalid("camera keyframe time must be finite"));
        }
        Ok(Self {
            time_seconds,
            camera,
        })
    }

    /// Local keyframe timestamp in seconds.
    #[must_use]
    pub const fn time_seconds(self) -> f64 {
        self.time_seconds
    }

    /// Camera state at the keyframe.
    #[must_use]
    pub const fn camera(self) -> Camera {
        self.camera
    }
}

/// Immutable camera keyframes sampled without allocation.
#[derive(Clone, Debug)]
pub struct CameraPath {
    keyframes: Arc<[CameraKeyframe]>,
    easing: CameraEasing,
}

impl CameraPath {
    /// Builds a path with at least two strictly increasing compatible cameras.
    ///
    /// Perspective and orthographic keyframes cannot mix inside one path; an
    /// explicit cut between projection models avoids a hidden discontinuity.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTimeline`] for malformed ordering or mixed
    /// projection models.
    pub fn new(keyframes: Arc<[CameraKeyframe]>, easing: CameraEasing) -> Result<Self, CoreError> {
        if keyframes.len() < 2
            || keyframes
                .windows(2)
                .any(|rows| rows[0].time_seconds >= rows[1].time_seconds)
        {
            return Err(invalid(
                "camera path needs two strictly increasing keyframes",
            ));
        }
        if keyframes.windows(2).any(|rows| {
            std::mem::discriminant(&rows[0].camera.projection)
                != std::mem::discriminant(&rows[1].camera.projection)
        }) {
            return Err(invalid("camera path projection models must match"));
        }
        Ok(Self { keyframes, easing })
    }

    /// Samples a finite local timestamp, clamped to the path endpoints.
    ///
    /// Segment lookup is `O(log keyframes)` and performs no allocation.
    #[must_use]
    pub fn sample(&self, time_seconds: f64) -> Option<Camera> {
        if !time_seconds.is_finite() {
            return None;
        }
        let first = *self.keyframes.first()?;
        let last = *self.keyframes.last()?;
        if time_seconds <= first.time_seconds {
            return Some(first.camera);
        }
        if time_seconds >= last.time_seconds {
            return Some(last.camera);
        }
        let upper = self
            .keyframes
            .partition_point(|keyframe| keyframe.time_seconds <= time_seconds);
        let start = *self.keyframes.get(upper.saturating_sub(1))?;
        let end = *self.keyframes.get(upper)?;
        let phase = num_traits::cast(
            (time_seconds - start.time_seconds) / (end.time_seconds - start.time_seconds),
        )?;
        Some(interpolate(
            start.camera,
            end.camera,
            self.easing.apply(phase),
        ))
    }

    /// Borrowed keyframes in timestamp order.
    #[must_use]
    pub fn keyframes(&self) -> &[CameraKeyframe] {
        &self.keyframes
    }

    /// Closed local interval covered by the path.
    #[must_use]
    pub fn range(&self) -> [f64; 2] {
        self.keyframes
            .first()
            .zip(self.keyframes.last())
            .map_or([0.0; 2], |(first, last)| {
                [first.time_seconds, last.time_seconds]
            })
    }
}

impl CameraEasing {
    fn apply(self, value: f32) -> f32 {
        let value = value.clamp(0.0, 1.0);
        match self {
            Self::Linear => value,
            Self::SmoothStep => value * value * (3.0 - 2.0 * value),
        }
    }
}

/// A lightweight camera plus global-timeline bookmark.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct CameraBookmark {
    label: Box<str>,
    time_seconds: f64,
    camera: Camera,
}

impl CameraBookmark {
    /// Captures a named camera at one finite global timeline timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTimeline`] for an empty label or malformed
    /// time/camera.
    pub fn new(
        label: impl Into<Box<str>>,
        time_seconds: f64,
        camera: Camera,
    ) -> Result<Self, CoreError> {
        let label = label.into();
        if label.trim().is_empty() || !time_seconds.is_finite() {
            return Err(invalid("camera bookmark label and time must be valid"));
        }
        validate_camera(camera)?;
        Ok(Self {
            label,
            time_seconds,
            camera,
        })
    }

    /// Restores the timeline first and updates the camera only after success.
    ///
    /// # Errors
    ///
    /// Propagates timeline validation without partially changing the camera.
    pub fn restore(
        &self,
        timeline: &mut Timeline,
        scene: &mut Scene,
        camera: &mut Camera,
    ) -> Result<(), CoreError> {
        timeline.apply(scene, self.time_seconds)?;
        *camera = self.camera;
        Ok(())
    }

    /// Human-readable bookmark label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Captured global timeline timestamp.
    #[must_use]
    pub const fn time_seconds(&self) -> f64 {
        self.time_seconds
    }

    /// Captured camera state.
    #[must_use]
    pub const fn camera(&self) -> Camera {
        self.camera
    }

    /// Encodes the lightweight bookmark as JSON metadata.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSceneDescription`] if encoding fails.
    pub fn to_json(&self) -> Result<String, CoreError> {
        serde_json::to_string(self).map_err(|error| CoreError::InvalidSceneDescription {
            summary: error.to_string(),
        })
    }

    /// Decodes and validates bookmark JSON metadata.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed JSON or invalid camera state.
    pub fn from_json(source: &str) -> Result<Self, CoreError> {
        let value: Self =
            serde_json::from_str(source).map_err(|error| CoreError::InvalidSceneDescription {
                summary: error.to_string(),
            })?;
        Self::new(value.label, value.time_seconds, value.camera)
    }
}

fn interpolate(start: Camera, end: Camera, phase: f32) -> Camera {
    let rotation = orientation(start)
        .slerp(orientation(end), phase)
        .normalize();
    let target = start.target.lerp(end.target, phase);
    let distance = start.focus_distance() + (end.focus_distance() - start.focus_distance()) * phase;
    let forward = rotation * -Vec3::Z;
    Camera {
        eye: target - forward * distance,
        target,
        up: rotation * Vec3::Y,
        projection: interpolate_projection(start.projection, end.projection, phase),
    }
}

fn orientation(camera: Camera) -> Quat {
    let forward = (camera.target - camera.eye).normalize();
    let right = forward.cross(camera.up).normalize();
    let up = right.cross(forward).normalize();
    Quat::from_mat3(&Mat3::from_cols(right, up, -forward)).normalize()
}

fn interpolate_projection(start: Projection, end: Projection, phase: f32) -> Projection {
    let blend = |left: f32, right: f32| left + (right - left) * phase;
    match (start, end) {
        (
            Projection::Perspective {
                fov_y: start_fov,
                aspect: start_aspect,
                near: start_near,
                far: start_far,
            },
            Projection::Perspective {
                fov_y: end_fov,
                aspect: end_aspect,
                near: end_near,
                far: end_far,
            },
        ) => Projection::Perspective {
            fov_y: blend(start_fov, end_fov),
            aspect: blend(start_aspect, end_aspect),
            near: blend(start_near, end_near),
            far: blend(start_far, end_far),
        },
        (
            Projection::Orthographic {
                height: start_height,
                aspect: start_aspect,
                near: start_near,
                far: start_far,
            },
            Projection::Orthographic {
                height: end_height,
                aspect: end_aspect,
                near: end_near,
                far: end_far,
            },
        ) => Projection::Orthographic {
            height: blend(start_height, end_height),
            aspect: blend(start_aspect, end_aspect),
            near: blend(start_near, end_near),
            far: blend(start_far, end_far),
        },
        _ => start,
    }
}

fn validate_camera(camera: Camera) -> Result<(), CoreError> {
    let forward = camera.target - camera.eye;
    if !camera.eye.is_finite()
        || !camera.target.is_finite()
        || !camera.up.is_finite()
        || forward.length_squared() <= f32::EPSILON
        || camera.up.length_squared() <= f32::EPSILON
        || forward.cross(camera.up).length_squared() <= f32::EPSILON
        || !valid_projection(camera.projection)
    {
        return Err(invalid("camera path contains a malformed camera"));
    }
    Ok(())
}

fn valid_projection(projection: Projection) -> bool {
    match projection {
        Projection::Perspective {
            fov_y,
            aspect,
            near,
            far,
        } => {
            [fov_y, aspect, near, far].into_iter().all(f32::is_finite)
                && fov_y > 0.0
                && fov_y < std::f32::consts::PI
                && aspect > 0.0
                && near > 0.0
                && near < far
        }
        Projection::Orthographic {
            height,
            aspect,
            near,
            far,
        } => {
            [height, aspect, near, far].into_iter().all(f32::is_finite)
                && height > 0.0
                && aspect > 0.0
                && near > 0.0
                && near < far
        }
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidTimeline { reason }
}
