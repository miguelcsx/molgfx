//! A stateful orbit camera: orbit, pan, dolly, roll, clip, projection switch and
//! a home framing, with the depth range refitted to the scene after every move.
//! Pure functions of pointer deltas, so it is testable without a GPU.

use molgfx_math::{BoundingSphere, Camera, Projection, Quat, Vec3};

/// The field of view a perspective projection has, or a sensible one for a view
/// that began orthographic.
fn fov_of(projection: &Projection) -> f32 {
    match *projection {
        Projection::Perspective { fov_y, .. } | Projection::OffAxis { fov_y, .. } => fov_y,
        Projection::Orthographic { .. } => 0.6,
    }
}

/// Radians per pixel of pointer travel.
const ROTATE_SPEED: f32 = 0.008;
/// Distance factor per scroll notch.
const ZOOM_STEP: f32 = 0.9;
/// Pan distance per pixel, as a fraction of the focus distance.
const PAN_SPEED: f32 = 1.5e-3;
/// The thinnest slab a clip may leave, ångström.
const MIN_SLAB: f32 = 0.5;
/// Closest the eye may come to the focus point.
const MIN_DISTANCE: f32 = 0.05;

/// A camera that orbits, pans and dollies, and can return to where it began.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitCamera {
    camera: Camera,
    home: Camera,
    /// The scene as a sphere, from the framing camera. The depth range is fitted
    /// to it after every move: a near plane fixed at the framing distance
    /// slices the molecule away as the eye comes closer.
    bound: BoundingSphere,
    /// Orthographic rather than perspective: size does not change with depth.
    ortho: bool,
    /// The perspective field of view, kept so the view can go back to it.
    fov_y: f32,
    /// How far the near plane is pushed into the scene, ångström (a front clip).
    clip_front: f32,
    /// How far the far plane is pulled toward the eye, ångström (a back clip).
    clip_back: f32,
}

/// The sphere a framing camera was built around: it looks at the scene's centre
/// and its depth range was fitted to the scene's extent.
fn bound_of(camera: &Camera) -> BoundingSphere {
    let radius = match camera.projection {
        Projection::Perspective { near, far, .. }
        | Projection::OffAxis { near, far, .. }
        | Projection::Orthographic { near, far, .. } => ((far - near) * 0.5).max(1.0),
    };
    BoundingSphere {
        center: camera.target,
        radius,
    }
}

impl OrbitCamera {
    /// Starts at `camera`, which is also the home position.
    #[must_use]
    pub fn new(camera: Camera) -> Self {
        Self {
            camera,
            home: camera,
            bound: bound_of(&camera),
            ortho: false,
            fov_y: fov_of(&camera.projection),
            clip_front: 0.0,
            clip_back: 0.0,
        }
    }

    /// Refits near and far so the whole scene stays inside the depth range from
    /// wherever the eye now is.
    fn refit(&mut self) {
        self.camera
            .projection
            .fit_near_far(self.camera.eye, &self.bound);
        self.apply_clip();
        self.sync_projection();
    }

    /// Pushes the near plane in and the far plane out by the clips, never past
    /// each other: what is left is a slab of at least half an ångström.
    fn apply_clip(&mut self) {
        if self.clip_front <= 0.0 && self.clip_back <= 0.0 {
            return;
        }
        let (front, back) = (self.clip_front, self.clip_back);
        match &mut self.camera.projection {
            Projection::Perspective { near, far, .. }
            | Projection::OffAxis { near, far, .. }
            | Projection::Orthographic { near, far, .. } => {
                let (n, f) = (*near, *far);
                let new_near = (n + front).min(f - MIN_SLAB);
                let new_far = (f - back).max(new_near + MIN_SLAB);
                *near = new_near.max(n);
                *far = new_far.min(f).max(*near + MIN_SLAB);
            }
        }
    }

    /// Clips the front and the back of the view by this much each, ångström;
    /// zero is no clip. The slab that is left stays between the planes as the
    /// camera moves.
    pub fn set_clip(&mut self, front: f32, back: f32) {
        self.clip_front = if front.is_finite() {
            front.max(0.0)
        } else {
            0.0
        };
        self.clip_back = if back.is_finite() { back.max(0.0) } else { 0.0 };
        self.refit();
    }

    /// The front and back clips, ångström.
    #[must_use]
    pub const fn clip(&self) -> (f32, f32) {
        (self.clip_front, self.clip_back)
    }

    /// The depth of the whole scene from the eye, ångström: what the clips are
    /// taken out of.
    #[must_use]
    pub fn depth(&self) -> f32 {
        self.bound.radius * 2.0
    }

    /// Keeps an orthographic view showing what a perspective one would at the
    /// focus plane, so zooming still changes the size of things.
    fn sync_projection(&mut self) {
        let (aspect, near, far) = match self.camera.projection {
            Projection::Perspective {
                aspect, near, far, ..
            }
            | Projection::OffAxis {
                aspect, near, far, ..
            }
            | Projection::Orthographic {
                aspect, near, far, ..
            } => (aspect, near, far),
        };
        self.camera.projection = if self.ortho {
            Projection::Orthographic {
                height: 2.0 * self.distance() * (self.fov_y * 0.5).tan(),
                aspect,
                near,
                far,
            }
        } else {
            Projection::Perspective {
                fov_y: self.fov_y,
                aspect,
                near,
                far,
            }
        };
    }

    /// Switches between perspective and orthographic projection.
    pub fn set_orthographic(&mut self, on: bool) {
        self.ortho = on;
        self.refit();
    }

    /// Whether the view is orthographic.
    #[must_use]
    pub const fn is_orthographic(&self) -> bool {
        self.ortho
    }

    /// The camera to render with.
    #[must_use]
    pub const fn camera(&self) -> &Camera {
        &self.camera
    }

    /// Follows the viewport's aspect ratio without moving the eye.
    pub fn set_aspect(&mut self, aspect: f32) {
        if aspect.is_finite() && aspect > 0.0 {
            self.camera.projection.set_aspect(aspect);
            self.home.projection.set_aspect(aspect);
        }
    }

    /// Tumbles about the focus point: yaw about the camera's up, pitch about
    /// its right axis, no fixed pole.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        let cam = &mut self.camera;
        let offset = cam.eye - cam.target;
        let view_dir = -offset.normalize_or_zero();
        let right = view_dir.cross(cam.up).normalize_or_zero();
        let yaw = Quat::from_axis_angle(cam.up, -dx * ROTATE_SPEED);
        let pitch = Quat::from_axis_angle(right, -dy * ROTATE_SPEED);
        let rotation = yaw * pitch;
        cam.eye = cam.target + rotation * offset;
        cam.up = (rotation * cam.up).normalize_or_zero();
        self.refit();
    }

    /// Turns the view about the line of sight (in the plane of the screen), by
    /// `radians`, positive anticlockwise as seen. The molecule does not move; the
    /// camera's up does.
    pub fn roll(&mut self, radians: f32) {
        let cam = &mut self.camera;
        let view_dir = (cam.target - cam.eye).normalize_or_zero();
        if view_dir.length() < 0.5 || !radians.is_finite() {
            return;
        }
        let turn = Quat::from_axis_angle(view_dir, -radians);
        cam.up = (turn * cam.up).normalize_or_zero();
        self.refit();
    }

    /// Moves the eye toward the focus point by `factor` (above one: closer, as a
    /// pinch that spreads the fingers does).
    pub fn zoom_by(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            let cam = &mut self.camera;
            let offset = cam.eye - cam.target;
            let distance = (offset.length() / factor).max(MIN_DISTANCE);
            cam.eye = cam.target + offset.normalize_or_zero() * distance;
            self.refit();
        }
    }

    /// Slides the eye and focus point together in the view plane.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let cam = &mut self.camera;
        let view_dir = (cam.target - cam.eye).normalize_or_zero();
        let right = view_dir.cross(cam.up).normalize_or_zero();
        let up = right.cross(view_dir);
        let scale = cam.focus_distance() * PAN_SPEED;
        let shift: Vec3 = (-right * dx + up * dy) * scale;
        cam.eye += shift;
        cam.target += shift;
        self.refit();
    }

    /// Moves the eye toward (positive) or away from (negative) the focus point.
    pub fn zoom(&mut self, scroll: f32) {
        let cam = &mut self.camera;
        let offset = cam.eye - cam.target;
        let distance = (offset.length() * ZOOM_STEP.powf(scroll)).max(MIN_DISTANCE);
        cam.eye = cam.target + offset.normalize_or_zero() * distance;
        self.refit();
    }

    /// Back to the framing camera.
    pub fn reset(&mut self) {
        // The framing camera's own depth range already fits its position.
        self.camera = self.home;
        if self.clip_front > 0.0 || self.clip_back > 0.0 {
            self.refit();
        } else {
            self.sync_projection();
        }
    }

    /// Looks at `target` from `distance`, keeping the viewing direction and
    /// the up vector. The depth range follows.
    pub fn fly_to(&mut self, target: Vec3, distance: f32) {
        let cam = &mut self.camera;
        let direction = (cam.eye - cam.target).normalize_or_zero();
        let direction = if direction.length() < 0.5 {
            Vec3::new(0.0, 0.0, 1.0)
        } else {
            direction
        };
        cam.target = target;
        cam.eye = target + direction * distance.max(MIN_DISTANCE);
        self.refit();
    }

    /// Re-centres on `point` without changing the distance or the direction.
    pub fn centre_on(&mut self, point: Vec3) {
        let distance = (self.camera.eye - self.camera.target).length();
        self.fly_to(point, distance);
    }

    /// Puts the eye, focus point and up vector back where they were saved. The
    /// depth range follows.
    pub fn set_pose(&mut self, eye: Vec3, target: Vec3, up: Vec3) {
        self.camera.eye = eye;
        self.camera.target = target;
        self.camera.up = up;
        self.refit();
    }

    /// Replaces both the current and the home camera (a new structure).
    pub fn reframe(&mut self, camera: Camera) {
        self.camera = camera;
        self.home = camera;
        self.bound = bound_of(&camera);
        self.fov_y = fov_of(&camera.projection);
        self.sync_projection();
    }

    /// Turns the camera toward a new framing (what is showing changed) without
    /// turning the molecule: the viewing direction and up stay, the focus point
    /// and distance come from `framing`, and so does the home position and the
    /// depth range.
    pub fn refocus(&mut self, framing: Camera) {
        let direction = (self.camera.eye - self.camera.target).normalize_or_zero();
        let direction = if direction.length() < 0.5 {
            Vec3::new(0.0, 0.0, 1.0)
        } else {
            direction
        };
        let distance = (framing.eye - framing.target).length().max(MIN_DISTANCE);
        self.camera.target = framing.target;
        self.camera.eye = framing.target + direction * distance;
        self.home = framing;
        self.bound = bound_of(&framing);
        self.refit();
    }

    /// Whether the focus point and distance are still the framing's own: nobody has
    /// zoomed or panned since the molecule was framed (turning does not count).
    #[must_use]
    pub fn at_framing(&self) -> bool {
        let home_distance = (self.home.eye - self.home.target).length();
        let tolerance = (home_distance * 1e-3).max(1e-4);
        (self.camera.target - self.home.target).length() <= tolerance
            && (self.distance() - home_distance).abs() <= tolerance
    }

    /// Distance from the eye to the focus point.
    #[must_use]
    pub fn distance(&self) -> f32 {
        (self.camera.eye - self.camera.target).length()
    }
}

#[cfg(test)]
#[path = "orbit_camera_tests.rs"]
mod tests;
