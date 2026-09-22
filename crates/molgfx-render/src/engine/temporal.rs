//! Deterministic temporal camera state and low-discrepancy subpixel sampling.

use super::{IllustrationStyle, QualityTier};
use crate::scene_gpu::{FrameUniforms, TemporalFrame};
use molgfx_math::{Camera, Mat4};

/// Subpixel offsets in pixels, one per accumulated sample.
///
/// The Halton(2, 3) low-discrepancy sequence, shifted to centre on the pixel.
/// The table is 64 long to match the quality accumulation budget: samples index
/// it as `frame_index % len`, so a shorter table would make the deep quality
/// pass revisit the same handful of offsets and stop refining coverage after
/// the first cycle, paying for samples that land exactly where earlier ones
/// did. Realtime accumulation uses the leading entries, and Halton is
/// progressive — every prefix is itself well distributed — so the short pass
/// stays even without a second table.
const JITTER: [[f32; 2]; 64] = [
    [0.000_000, -0.166_667],
    [-0.250_000, 0.166_667],
    [0.250_000, -0.388_889],
    [-0.375_000, -0.055_556],
    [0.125_000, 0.277_778],
    [-0.125_000, -0.277_778],
    [0.375_000, 0.055_556],
    [-0.437_500, 0.388_889],
    [0.062_500, -0.462_963],
    [-0.187_500, -0.129_630],
    [0.312_500, 0.203_704],
    [-0.312_500, -0.351_852],
    [0.187_500, -0.018_519],
    [-0.062_500, 0.314_815],
    [0.437_500, -0.240_741],
    [-0.468_750, 0.092_593],
    [0.031_250, 0.425_926],
    [-0.218_750, -0.425_926],
    [0.281_250, -0.092_593],
    [-0.343_750, 0.240_741],
    [0.156_250, -0.314_815],
    [-0.093_750, 0.018_519],
    [0.406_250, 0.351_852],
    [-0.406_250, -0.203_704],
    [0.093_750, 0.129_630],
    [-0.156_250, 0.462_963],
    [0.343_750, -0.487_654],
    [-0.281_250, -0.154_321],
    [0.218_750, 0.179_012],
    [-0.031_250, -0.376_543],
    [0.468_750, -0.043_210],
    [-0.484_375, 0.290_123],
    [0.015_625, -0.265_432],
    [-0.234_375, 0.067_901],
    [0.265_625, 0.401_235],
    [-0.359_375, -0.450_617],
    [0.140_625, -0.117_284],
    [-0.109_375, 0.216_049],
    [0.390_625, -0.339_506],
    [-0.421_875, -0.006_173],
    [0.078_125, 0.327_160],
    [-0.171_875, -0.228_395],
    [0.328_125, 0.104_938],
    [-0.296_875, 0.438_272],
    [0.203_125, -0.413_580],
    [-0.046_875, -0.080_247],
    [0.453_125, 0.253_086],
    [-0.453_125, -0.302_469],
    [0.046_875, 0.030_864],
    [-0.203_125, 0.364_198],
    [0.296_875, -0.191_358],
    [-0.328_125, 0.141_975],
    [0.171_875, 0.475_309],
    [-0.078_125, -0.475_309],
    [0.421_875, -0.141_975],
    [-0.390_625, 0.191_358],
    [0.109_375, -0.364_198],
    [-0.140_625, -0.030_864],
    [0.359_375, 0.302_469],
    [-0.265_625, -0.253_086],
    [0.234_375, 0.080_247],
    [-0.015_625, 0.413_580],
    [0.484_375, -0.438_272],
    [-0.492_188, -0.104_938],
];

#[derive(Debug, Default)]
pub(crate) struct TemporalState {
    frame_index: u32,
    settled_frames: u8,
    tier: QualityTier,
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
    /// The tier the caller last published into this state.
    pub(crate) const fn tier(&self) -> QualityTier {
        self.tier
    }

    /// Records the tier the frame loop is rendering at.
    pub(crate) fn set_tier(&mut self, tier: QualityTier) {
        self.tier = tier;
    }

    pub(crate) fn reset(&mut self) {
        self.frame_index = 0;
        self.settled_frames = 0;
        self.previous_view_proj = None;
        self.previous_camera = None;
        self.write_index = 0;
    }

    pub(crate) fn invalidate_convergence(&mut self) {
        self.settled_frames = 0;
    }

    pub(crate) fn prepare(&mut self, camera: &Camera, options: &TemporalOptions) -> FrameUniforms {
        if self.camera_changed(camera) {
            self.invalidate_convergence();
        }
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
                npr: options.illustration.npr_packed(),
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
        self.settled_frames = self.settled_frames.saturating_add(1);
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

    pub(crate) const fn needs_another_frame(&self, sample_budget: u8) -> bool {
        self.settled_frames < sample_budget
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
