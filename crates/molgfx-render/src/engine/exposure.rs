//! Frozen output state shared by images, sequences, HDR and profiling.

use super::image::ImagePurpose;
use super::{Engine, MotionBlur, QualityTier, TemporalOptions};
use crate::RenderError;
use molgfx_core::Scene;
#[cfg(not(target_arch = "wasm32"))]
use molgfx_gpu::Queue;
use molgfx_gpu::{Device, FenceValue};
use molgfx_math::Camera;

/// Samples recorded into one command buffer before it is submitted.
///
/// Every render encoder holds driver memory until its command buffer retires,
/// so a publication exposure that queues all of its samples at once keeps
/// hundreds of encoders alive together. Runs of this size, with at most two
/// in flight, bound that without leaving the device idle.
pub(super) const EXPOSURE_RUN: u32 = 4;

pub(super) struct Exposure {
    pub(super) samples: u32,
    submitted: u32,
    /// The previous run, awaited before the next is queued (native only).
    #[cfg(not(target_arch = "wasm32"))]
    in_flight: Option<FenceValue>,
    bank: usize,
    camera: Camera,
    options: TemporalOptions,
}

impl<D: Device> Engine<D> {
    pub(super) fn prepare_exposure(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        purpose: ImagePurpose,
    ) -> Result<Exposure, RenderError> {
        let preparation = self.prepare_image(scene, purpose)?;
        let scene_reset = self.temporal_scene_identity.replace(scene.cache_identity())
            != Some(scene.cache_identity());
        let publication = purpose == ImagePurpose::Publication;
        let samples = if publication {
            self.tier().image_samples()
        } else {
            1
        };
        if samples == 0 || samples as usize > crate::scene_gpu::exposure_arena::EXPOSURE_SAMPLES {
            return Err(RenderError::Residency {
                reason: "exposure exceeds its uniform arena budget",
            });
        }
        let bank = self
            .scene_gpu
            .acquire_exposure_bank(&self.device, &self.queue)?;
        self.scene_gpu.begin_frame();
        let optics = self.resolve_optics(scene, camera)?;
        let shadow = self.shadow_bound.fit(
            scene,
            camera,
            self.resolved_plan.lighting(),
            preparation.scene_changed || preparation.rebuild,
        );
        Ok(Exposure {
            samples,
            submitted: 0,
            #[cfg(not(target_arch = "wasm32"))]
            in_flight: None,
            bank,
            camera: *camera,
            options: TemporalOptions {
                extent: [self.width, self.height],
                reset: publication || scene_reset || preparation.rebuild,
                quality: self.tier() >= QualityTier::Standard,
                publication,
                illustration: self.resolved_plan.illustration(),
                depth_cue: self.resolved_plan.packed_depth_cue(),
                optics,
                motion_blur: self
                    .resolved_plan
                    .motion_blur()
                    .map_or([0.0; 4], MotionBlur::packed),
                atmosphere: self
                    .resolved_plan
                    .packed_presentation(self.scene_gpu.has_translucency()),
                lighting: self.resolved_plan.packed_lighting(),
                shadow_view: shadow.view,
                shadow_projection: shadow.projection,
                shadow_view_proj: shadow.view_projection,
            },
        })
    }

    pub(super) fn prepare_exposure_sample(
        &mut self,
        exposure: &mut Exposure,
        sample: u32,
    ) -> Result<(), RenderError> {
        if sample >= exposure.samples {
            return Err(RenderError::Residency {
                reason: "exposure sample is outside its budget",
            });
        }
        let uniforms = self.temporal.prepare(&exposure.camera, &exposure.options);
        exposure.options.reset = false;
        if sample == 0 {
            self.scene_gpu
                .write_frame_uniforms(&self.queue, &uniforms)?;
        }
        self.scene_gpu
            .stage_exposure_uniforms(exposure.bank, sample as usize, &uniforms)?;
        if exposure.options.publication && sample + 1 == exposure.samples {
            self.temporal_scene_identity = None;
        }
        Ok(())
    }

    /// Submits the samples recorded so far once a run is full, and starts a
    /// fresh encoder for the rest. The last run goes through
    /// [`Self::submit_exposure`].
    pub(super) fn flush_exposure_run(
        &mut self,
        exposure: &mut Exposure,
        sample: u32,
        encoder: &mut D::CommandEncoder,
    ) -> Result<(), RenderError> {
        let recorded = sample + 1;
        if !recorded.is_multiple_of(EXPOSURE_RUN) || recorded >= exposure.samples {
            return Ok(());
        }
        let full = std::mem::replace(encoder, self.device.create_command_encoder());
        let fence = self.scene_gpu.flush_exposure_uniforms(
            &self.queue,
            exposure.bank,
            exposure.submitted as usize..recorded as usize,
            full,
        )?;
        exposure.submitted = recorded;
        // Native callers may block; a browser cannot, and keeps its queue.
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(previous) = exposure.in_flight.replace(fence) {
            self.queue
                .wait_fence_blocking(&self.device, previous)
                .map_err(RenderError::from)?;
        }
        #[cfg(target_arch = "wasm32")]
        let _ = fence;
        Ok(())
    }

    pub(super) fn submit_exposure(
        &mut self,
        exposure: &Exposure,
        encoder: D::CommandEncoder,
    ) -> Result<FenceValue, RenderError> {
        self.scene_gpu.submit_exposure_uniforms(
            &self.queue,
            exposure.bank,
            exposure.submitted as usize..exposure.samples as usize,
            encoder,
        )
    }
}
