//! Camera controllers: pure functions from input events to camera edits.
//!
//! Controllers know nothing about windows; they consume abstract events and
//! mutate a camera. Every update is `O(1)`.

use crate::input::{Button, InputEvent, Key};
use pdviewx_math::{Camera, Quat, Vec3};

#[cfg(test)]
#[path = "controller_tests.rs"]
mod tests;

/// Sensitivity shared by the rotating controllers, radians per pixel.
const ROTATE_SPEED: f32 = 0.008;
/// Zoom factor per scroll notch.
const ZOOM_STEP: f32 = 0.9;
/// Pan distance per pixel as a fraction of the focus distance.
const PAN_SPEED: f32 = 1.5e-3;

/// The inspection default: rotation about the target in any direction, no
/// fixed pole, so a molecule can be tumbled freely.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArcballController {
    dragging: bool,
    panning: bool,
    last: Option<(f32, f32)>,
}

/// Orbit: like arcball, but the horizon stays level (yaw around world +Y,
/// clamped pitch).
#[derive(Clone, Copy, Debug, Default)]
pub struct OrbitController {
    dragging: bool,
    panning: bool,
    last: Option<(f32, f32)>,
}

/// First-person flight: the eye moves, the view direction turns.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlyController {
    turning: bool,
    last: Option<(f32, f32)>,
    /// Held state per navigation key, indexed by `held_index`.
    held: [bool; 6],
}

/// The `held` array slot for a navigation key.
fn held_index(key: Key) -> usize {
    match key {
        Key::Forward => 0,
        Key::Backward => 1,
        Key::Left => 2,
        Key::Right => 3,
        Key::Up => 4,
        Key::Down => 5,
    }
}

/// Shared drag bookkeeping: returns the pixel delta when dragging.
fn drag_delta(last: &mut Option<(f32, f32)>, active: bool, x: f32, y: f32) -> Option<(f32, f32)> {
    if !active {
        *last = None;
        return None;
    }
    let delta = match *last {
        Some((lx, ly)) => (x - lx, y - ly),
        None => (0.0, 0.0),
    };
    *last = Some((x, y));
    Some(delta)
}

fn zoom(camera: &mut Camera, factor: f32) {
    let offset = camera.eye - camera.target;
    let distance = (offset.length() * factor).max(0.05);
    camera.eye = camera.target + offset.normalize_or_zero() * distance;
}

fn pan(camera: &mut Camera, dx: f32, dy: f32) {
    let view_dir = (camera.target - camera.eye).normalize_or_zero();
    let right = view_dir.cross(camera.up).normalize_or_zero();
    let up = right.cross(view_dir);
    let scale = camera.focus_distance() * PAN_SPEED;
    let shift = (-right * dx + up * dy) * scale;
    camera.eye += shift;
    camera.target += shift;
}

impl ArcballController {
    /// Feeds one event; mutates the camera in place.
    pub fn update(&mut self, event: InputEvent, camera: &mut Camera) {
        match event {
            InputEvent::PointerButton {
                button: Button::Left,
                pressed,
                ..
            } => {
                self.dragging = pressed;
                self.last = None;
            }
            InputEvent::PointerButton {
                button: Button::Right | Button::Middle,
                pressed,
                ..
            } => {
                self.panning = pressed;
                self.last = None;
            }
            InputEvent::PointerMove { x, y } => {
                if self.panning {
                    if let Some((dx, dy)) = drag_delta(&mut self.last, true, x, y) {
                        pan(camera, dx, dy);
                    }
                } else if let Some((dx, dy)) = drag_delta(&mut self.last, self.dragging, x, y) {
                    // Tumble: yaw around the camera's own up, pitch around
                    // its right axis; no fixed pole.
                    let offset = camera.eye - camera.target;
                    let view_dir = -offset.normalize_or_zero();
                    let right = view_dir.cross(camera.up).normalize_or_zero();
                    let yaw = Quat::from_axis_angle(camera.up, -dx * ROTATE_SPEED);
                    let pitch = Quat::from_axis_angle(right, -dy * ROTATE_SPEED);
                    let rotation = yaw * pitch;
                    camera.eye = camera.target + rotation * offset;
                    camera.up = (rotation * camera.up).normalize_or_zero();
                }
            }
            InputEvent::Scroll { delta } => zoom(camera, ZOOM_STEP.powf(delta)),
            InputEvent::Pinch { scale } => {
                if scale > 0.0 {
                    zoom(camera, 1.0 / scale);
                }
            }
            InputEvent::Key { .. } => {}
        }
    }
}

impl OrbitController {
    /// Feeds one event; mutates the camera in place, keeping +Y up.
    pub fn update(&mut self, event: InputEvent, camera: &mut Camera) {
        match event {
            InputEvent::PointerButton {
                button: Button::Left,
                pressed,
                ..
            } => {
                self.dragging = pressed;
                self.last = None;
            }
            InputEvent::PointerButton {
                button: Button::Right | Button::Middle,
                pressed,
                ..
            } => {
                self.panning = pressed;
                self.last = None;
            }
            InputEvent::PointerMove { x, y } => {
                if self.panning {
                    if let Some((dx, dy)) = drag_delta(&mut self.last, true, x, y) {
                        pan(camera, dx, dy);
                    }
                } else if let Some((dx, dy)) = drag_delta(&mut self.last, self.dragging, x, y) {
                    let offset = camera.eye - camera.target;
                    let radius = offset.length().max(1e-4);
                    let mut yaw = offset.z.atan2(offset.x);
                    let mut pitch = (offset.y / radius).clamp(-1.0, 1.0).asin();
                    yaw += dx * ROTATE_SPEED;
                    pitch = (pitch + dy * ROTATE_SPEED).clamp(
                        -std::f32::consts::FRAC_PI_2 + 0.01,
                        std::f32::consts::FRAC_PI_2 - 0.01,
                    );
                    let (sp, cp) = pitch.sin_cos();
                    let (sy, cy) = yaw.sin_cos();
                    camera.eye = camera.target + Vec3::new(cp * cy, sp, cp * sy) * radius;
                    camera.up = Vec3::Y;
                }
            }
            InputEvent::Scroll { delta } => zoom(camera, ZOOM_STEP.powf(delta)),
            InputEvent::Pinch { scale } => {
                if scale > 0.0 {
                    zoom(camera, 1.0 / scale);
                }
            }
            InputEvent::Key { .. } => {}
        }
    }
}

impl FlyController {
    /// Feeds one event; drag turns, keys set the motion state.
    pub fn update(&mut self, event: InputEvent, camera: &mut Camera) {
        match event {
            InputEvent::PointerButton {
                button: Button::Left,
                pressed,
                ..
            } => {
                self.turning = pressed;
                self.last = None;
            }
            InputEvent::PointerMove { x, y } => {
                if let Some((dx, dy)) = drag_delta(&mut self.last, self.turning, x, y) {
                    let view_dir = (camera.target - camera.eye).normalize_or_zero();
                    let right = view_dir.cross(camera.up).normalize_or_zero();
                    let yaw = Quat::from_axis_angle(Vec3::Y, -dx * ROTATE_SPEED);
                    let pitch = Quat::from_axis_angle(right, -dy * ROTATE_SPEED);
                    let new_dir = (yaw * pitch * view_dir).normalize_or_zero();
                    camera.target = camera.eye + new_dir * camera.focus_distance();
                }
            }
            InputEvent::Key { key, pressed } => self.held[held_index(key)] = pressed,
            InputEvent::Scroll { delta } => zoom(camera, ZOOM_STEP.powf(delta)),
            InputEvent::Pinch { .. } | InputEvent::PointerButton { .. } => {}
        }
    }

    /// Advances flight by `dt` seconds at `speed` Ångström per second,
    /// applying the held keys.
    pub fn advance(&self, camera: &mut Camera, dt: f32, speed: f32) {
        let view_dir = (camera.target - camera.eye).normalize_or_zero();
        let right = view_dir.cross(camera.up).normalize_or_zero();
        let axes = [view_dir, -view_dir, -right, right, Vec3::Y, -Vec3::Y];
        let mut motion = Vec3::ZERO;
        for (held, axis) in self.held.iter().zip(axes) {
            if *held {
                motion += axis;
            }
        }
        let step = motion.normalize_or_zero() * speed * dt;
        camera.eye += step;
        camera.target += step;
    }
}
