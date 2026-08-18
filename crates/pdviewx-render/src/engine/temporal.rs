//! Deterministic temporal camera state and low-discrepancy subpixel sampling.

use super::IllustrationStyle;
use crate::scene_gpu::{FrameUniforms, TemporalFrame};
use pdviewx_math::{Camera, Mat4};

const JITTER: [[f32; 2]; 16] = [
    [0.0, -1.0 / 6.0],
    [-0.25, 1.0 / 6.0],
    [0.25, -7.0 / 18.0],
    [-0.375, -1.0 / 18.0],
    [0.125, 5.0 / 18.0],
    [-0.125, -5.0 / 18.0],
    [0.375, 1.0 / 18.0],
    [-0.4375, 7.0 / 18.0],
    [0.0625, -25.0 / 54.0],
    [-0.1875, -7.0 / 54.0],
    [0.3125, 11.0 / 54.0],
    [-0.3125, -19.0 / 54.0],
    [0.1875, -1.0 / 54.0],
    [-0.0625, 17.0 / 54.0],
    [0.4375, -13.0 / 54.0],
    [-0.46875, 5.0 / 54.0],
];

#[derive(Debug, Default)]
pub(crate) struct TemporalState {
    frame_index: u32,
    previous_view_proj: Option<Mat4>,
    previous_camera: Option<Camera>,
    write_index: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct TemporalOptions {
    pub(crate) extent: [u32; 2],
    pub(crate) reset: bool,
    pub(crate) quality: bool,
    pub(crate) publication: bool,
    pub(crate) illustration: IllustrationStyle,
    pub(crate) optics: [f32; 4],
    pub(crate) motion_blur: [f32; 4],
    pub(crate) atmosphere: [[f32; 4]; 6],
    pub(crate) lighting: [[f32; 4]; 8],
    pub(crate) shadow_view: Mat4,
    pub(crate) shadow_projection: Mat4,
    pub(crate) shadow_view_proj: Mat4,
}

impl TemporalState {
    pub(crate) fn reset(&mut self) {
        self.frame_index = 0;
        self.previous_view_proj = None;
        self.previous_camera = None;
        self.write_index = 0;
    }

    pub(crate) fn prepare(&mut self, camera: &Camera, options: &TemporalOptions) -> FrameUniforms {
        if options.reset
            || self
                .previous_camera
                .is_some_and(|old| camera_cut(old, *camera))
        {
            self.reset();
        }
        let jitter = JITTER[(self.frame_index as usize) % JITTER.len()];
        let uniforms = FrameUniforms::new(
            camera,
            options.extent[0],
            options.extent[1],
            &TemporalFrame {
                jitter_pixels: jitter,
                previous_view_proj: self.previous_view_proj,
                shadow_view: options.shadow_view,
                shadow_projection: options.shadow_projection,
                shadow_view_proj: options.shadow_view_proj,
                sample_index: self.frame_index,
                quality: options.quality,
                publication: options.publication,
                illustration: options.illustration.packed(camera.focus_distance()),
                optics: options.optics,
                motion_blur: options.motion_blur,
                atmosphere: options.atmosphere,
                lighting: options.lighting,
            },
        );
        self.write_index = (self.frame_index as usize) & 1;
        self.previous_view_proj = Some(uniforms.view_proj);
        self.previous_camera = Some(*camera);
        self.frame_index = self.frame_index.wrapping_add(1);
        uniforms
    }

    pub(crate) const fn write_index(&self) -> usize {
        self.write_index
    }

    pub(crate) fn camera_changed(&self, camera: &Camera) -> bool {
        match self.previous_camera {
            Some(previous) => previous != *camera,
            None => true,
        }
    }
}

fn camera_cut(previous: Camera, current: Camera) -> bool {
    let scale = previous
        .focus_distance()
        .max(current.focus_distance())
        .max(1.0);
    if previous.eye.distance(current.eye) > scale * 0.5
        || previous.target.distance(current.target) > scale * 0.5
    {
        return true;
    }
    let previous_direction = (previous.target - previous.eye).normalize_or_zero();
    let current_direction = (current.target - current.eye).normalize_or_zero();
    previous_direction.dot(current_direction) < std::f32::consts::FRAC_1_SQRT_2
}

#[cfg(test)]
#[path = "temporal_tests.rs"]
mod tests;
