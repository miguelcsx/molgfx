//! The frame loop: sync, build, record, one submission, present.
//!
//! CPU cost per frame is `O(passes)` — a fixed handful of pass records —
//! never a function of atom count; per-primitive work lives on the GPU.
//! Nothing here allocates on the steady-state path beyond the first frame's
//! pool construction.

use super::{Engine, FrameOutcome, MotionBlur, RenderMode, TemporalOptions, fit_shadow};
use crate::error::RenderError;
use crate::graph::{DisplayEncoding, PassContext, ResourceTable, TransientPool, plan_aliases};
use crate::passes::FrameBindings;
use pdviewx_core::Scene;
use pdviewx_gpu::Queue as _;
use pdviewx_gpu::{Device, Surface as _, SurfaceError, SurfaceFrame as _};
use pdviewx_math::Camera;

impl<D: Device> Engine<D> {
    /// Renders one frame of the scene to the presentation surface.
    ///
    /// A lost or outdated surface reconfigures and returns
    /// [`FrameOutcome::Skipped`]; the next frame recovers. Nothing panics on
    /// conditions a caller can hit.
    ///
    /// # Errors
    ///
    /// Device loss beyond surface recovery, or graph reconstruction
    /// failures.
    pub fn render(&mut self, scene: &Scene, camera: &Camera) -> Result<FrameOutcome, RenderError> {
        // Sync: upload only what changed since the last frame.
        let scene_changed = self.scene_gpu.sync(
            &self.device,
            &self.queue,
            scene,
            self.mode == RenderMode::Quality,
            [self.width, self.height],
        )?;

        // Build: (re)allocate the transient pool when the size changed.
        let rebuild = match &self.pool {
            Some(pool) => !pool.matches(self.width, self.height),
            None => true,
        };
        if rebuild {
            let plan = plan_aliases(&self.resources, &self.pass_nodes, &self.order);
            self.pool = Some(TransientPool::build(
                &self.device,
                &self.resources,
                plan,
                self.width,
                self.height,
            )?);
            self.bindings = self
                .pool
                .as_ref()
                .and_then(|pool| FrameBindings::new(&self.device, pool, &self.passes));
        }
        let camera_changed = self.temporal.camera_changed(camera);
        let quality = self.mode == RenderMode::Quality && !camera_changed;
        let optics = self.resolve_optics(scene, camera)?;
        let shadow = fit_shadow(scene, camera, self.resolved_plan.lighting());
        let uniforms = self.temporal.prepare(
            camera,
            &TemporalOptions {
                extent: [self.width, self.height],
                reset: scene_changed
                    || rebuild
                    || (self.mode == RenderMode::Quality && camera_changed),
                quality,
                publication: false,
                illustration: self.resolved_plan.illustration(),
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
        );
        self.scene_gpu.write_frame_uniforms(&self.queue, &uniforms);
        let frame = self.acquire_surface_frame()?;
        let Some(frame) = frame else {
            // Off-screen targets arrive with the image-render path.
            return Ok(FrameOutcome::Skipped);
        };
        let Some(pool) = &self.pool else {
            return Ok(FrameOutcome::Skipped);
        };
        let temporal_write = self.temporal.write_index();

        // Record every pass in schedule order into one encoder.
        let mut encoder = self.device.create_command_encoder();
        self.scene_gpu
            .record_particle_motion(&mut encoder, &self.passes.particle_motion);
        self.scene_gpu
            .record_trajectories(&mut encoder, &self.passes.trajectory, None);
        self.scene_gpu
            .record_surface_fields(&mut encoder, &self.passes.surface_field);
        {
            let table = ResourceTable {
                pool,
                swapchain: frame.view(),
            };
            for &index in &self.order {
                let Some(node) = self.pass_nodes.get(index) else {
                    continue;
                };
                let mut ctx = PassContext {
                    encoder: &mut encoder,
                    resources: &table,
                    passes: &self.passes,
                    bindings: self.bindings.as_ref(),
                    scene: &self.scene_gpu,
                    timestamps: None,
                    temporal_write,
                    quality,
                    depth_of_field: self.resolved_plan.depth_of_field().is_some(),
                    motion_blur: self.resolved_plan.motion_blur().is_some(),
                    display_encoding: self.display_encoding(),
                };
                (node.record)(&mut ctx);
            }
        }

        // One submission, then present.
        self.queue.submit(encoder);
        frame.present();
        Ok(FrameOutcome::Presented)
    }

    /// The display encoding this frame presents for.
    ///
    /// Selects a pre-built tonemap pipeline rather than a per-pixel branch, so
    /// the encoding is fixed for the whole frame by construction.
    pub(super) fn display_encoding(&self) -> DisplayEncoding {
        let display = self.resolved_plan.display();
        DisplayEncoding {
            gamut: display.gamut,
            transfer: display.transfer,
        }
    }

    fn acquire_surface_frame(
        &mut self,
    ) -> Result<Option<<D::Surface as pdviewx_gpu::Surface<D>>::Frame>, RenderError> {
        let Some(surface) = &mut self.surface else {
            return Ok(None);
        };
        match surface.acquire() {
            Ok(frame) => Ok(Some(frame)),
            Err(SurfaceError::Lost | SurfaceError::Outdated) => {
                surface.configure(
                    &self.device,
                    &pdviewx_gpu::SurfaceConfig {
                        width: self.width,
                        height: self.height,
                        format: self.target_format,
                    },
                );
                Ok(None)
            }
            Err(SurfaceError::Timeout) => Ok(None),
            Err(error) => Err(RenderError::Gpu(error.into())),
        }
    }
}
