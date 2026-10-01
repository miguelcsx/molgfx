//! Frozen output state shared by images, sequences, HDR and profiling.

use super::image::ImagePurpose;
use super::{Engine, MotionBlur, QualityTier, TemporalOptions};
use crate::RenderError;
use molgfx_core::Scene;
use molgfx_gpu::{Device, FenceValue};
use molgfx_math::Camera;

pub(super) struct Exposure {
    pub(super) samples: u32,
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

    pub(super) fn submit_exposure(
        &mut self,
        exposure: &Exposure,
        encoder: D::CommandEncoder,
    ) -> Result<FenceValue, RenderError> {
        self.scene_gpu.submit_exposure_uniforms(
            &self.queue,
            exposure.bank,
            exposure.samples as usize,
            encoder,
        )
    }
}
