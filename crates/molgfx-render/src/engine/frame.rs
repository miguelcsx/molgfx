//! The frame loop: sync, build, record, one submission, present.
//!
//! CPU cost per frame is proportional to the active passes, the placed
//! structures, the resident semantic tables and the representation states —
//! never to atom or bond count, because per-primitive work lives on the GPU.
//! A structure or representation whose revisions are unchanged costs one
//! comparison and no upload, so a still scene walks a short fixed list rather
//! than touching its contents. Nothing here allocates on the steady-state path
//! beyond the first frame's pool construction.

use super::{
    Engine, FrameCompleteness, FrameDegradation, FrameMetrics, FrameReport, FrameStatus,
    MotionBlur, QualityTier, RenderMode, TemporalOptions,
};
use crate::error::RenderError;
use crate::graph::{DisplayEncoding, PassContext, ResourceTable, TransientPool, plan_aliases};
use crate::passes::FrameBindings;
use molgfx_core::Scene;
use molgfx_gpu::Queue as _;
use molgfx_gpu::{Device, Surface as _, SurfaceError, SurfaceFrame as _};
use molgfx_math::Camera;
// Uses the host monotonic clock on both native and browser targets, matching
// the profiling path so the two frame-time sources stay comparable.
use web_time::Instant;

impl<D: Device> Engine<D> {
    /// Uploads everything the scene changed since the previous frame.
    ///
    /// Returns whether any of it changed, and restarts temporal accumulation
    /// when it did or while uploads are still in flight.
    fn sync_scene(&mut self, scene: &Scene) -> Result<bool, RenderError> {
        let scene_changed = self.scene_gpu.sync(crate::scene_gpu::SceneSync {
            device: &self.device,
            queue: &self.queue,
            scene,
            quality: self.tier() >= QualityTier::Standard,
            extent: [self.width, self.height],
            ray_query_layout: self.passes.ambient_occlusion.ray_query_layout(),
            derived_cache: &mut self.derived_cache,
            derived_frame: self.derived_frame,
        })?;
        self.chunk_residency.sync_scene(
            &mut self.scene_gpu,
            &self.device,
            &self.queue,
            &mut self.derived_cache,
            self.derived_frame,
        )?;
        self.derived_frame = self.derived_frame.wrapping_add(1);
        if scene_changed || self.chunk_residency.metrics().uploads.active_tickets != 0 {
            self.temporal.invalidate_convergence();
        }
        Ok(scene_changed)
    }

    /// Renders one frame of the scene to the presentation surface.
    ///
    /// A lost or outdated surface reconfigures and returns
    /// [`FrameStatus::Skipped`]; the next frame recovers. Nothing panics on
    /// conditions a caller can hit.
    ///
    /// # Errors
    ///
    /// Device loss beyond surface recovery, or graph reconstruction
    /// failures.
    pub fn render(&mut self, scene: &Scene, camera: &Camera) -> Result<FrameReport, RenderError> {
        let frame_start = Instant::now();
        self.sync_quality_tier();
        self.device.check_errors()?;
        self.chunk_residency.begin_epoch();
        self.scene_gpu.begin_frame();
        // Sync: upload only what changed since the last frame.
        let scene_changed = self.sync_scene(scene)?;

        // Build: (re)allocate the transient pool when the size changed.
        let rebuild = self.rebuild_pool_if_needed()?;
        // Settle: every generated pipeline this frame will draw is resolved
        // before any pass opens a render pass, so recording only reads what
        // this phase compiled.
        self.scene_gpu
            .settle_specializations(&self.device, scene, &self.passes);
        let camera_changed = self.temporal.camera_changed(camera);
        let identity = scene.cache_identity();
        let scene_reset = self.temporal_scene_identity.replace(identity) != Some(identity);
        let cinematic = self.tier() >= QualityTier::Standard;
        let optics = self.resolve_optics(scene, camera)?;
        let shadow =
            self.shadow_bound
                .fit(scene, camera, self.resolved_plan.lighting(), scene_changed);
        let uniforms = self.temporal.prepare(
            camera,
            &TemporalOptions {
                extent: [self.width, self.height],
                reset: scene_reset
                    || rebuild
                    || (self.mode == RenderMode::Cinematic && camera_changed),
                quality: cinematic,
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
        self.scene_gpu
            .write_frame_uniforms(&self.queue, &uniforms)?;
        let frame = self.acquire_surface_frame()?;
        let Some(frame) = frame else {
            // Off-screen targets arrive with the image-render path.
            return Ok(self.frame_report(FrameStatus::Skipped));
        };
        let temporal_write = self.temporal.write_index();

        // Record every pass in schedule order into one encoder.
        let mut encoder = self.device.create_command_encoder();
        self.scene_gpu
            .settle_specializations(&self.device, scene, &self.passes);
        self.record_scene_compute(&mut encoder, cinematic);
        let Some(pool) = &self.pool else {
            return Ok(self.frame_report(FrameStatus::Skipped));
        };
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
                    quality: cinematic,
                    display_encoding: self.display_encoding(),
                };
                (node.record)(&mut ctx);
            }
        }

        // One submission, then present.
        self.queue.submit(encoder);
        self.device.check_errors()?;
        frame.present();
        // The report describes the frame that was just rendered, so it is
        // captured before the loop advances.
        let report = self.frame_report(FrameStatus::Presented);
        // Close the loop: this frame's real duration decides the tier the next
        // frame builds at. That frame republishes the tier at its top, which is
        // also where a move restarts accumulation.
        // Kept in u64 throughout: a frame that outruns u64 nanoseconds is
        // already slower than any tier can act on, so the arithmetic saturates.
        let frame_time = frame_start.elapsed();
        let elapsed = frame_time
            .as_secs()
            .saturating_mul(1_000_000_000)
            .saturating_add(u64::from(frame_time.subsec_nanos()));
        self.adaptive.observe(elapsed);
        Ok(report)
    }

    fn record_scene_compute(&mut self, encoder: &mut D::CommandEncoder, cinematic: bool) {
        self.passes
            .cull
            .record_attribute_timelines(&self.scene_gpu, encoder);
        self.passes
            .cull
            .record_instance_timelines(&self.scene_gpu, encoder);
        let point_coordinates_changed = self
            .passes
            .cull
            .record_point_timelines(&self.scene_gpu, encoder);
        self.scene_gpu
            .record_particle_motion(encoder, &self.passes.particle_motion);
        let structure_coordinates_changed =
            self.scene_gpu
                .record_trajectories(encoder, &self.passes.trajectory, None);
        let paged_coordinates_changed = self
            .passes
            .cull
            .record_paged_trajectories(&self.scene_gpu, encoder);
        self.scene_gpu.record_dynamic_relations(
            encoder,
            &self.passes.relation_resolve,
            structure_coordinates_changed || paged_coordinates_changed || point_coordinates_changed,
        );
        self.scene_gpu
            .record_occupancies(encoder, &self.passes.occupancy);
        self.scene_gpu.record_surface_fields(
            encoder,
            &self.passes.surface_field,
            &self.passes.surface_components,
        );
        self.scene_gpu.record_quality_hardware(encoder, cinematic);
    }

    fn frame_report(&self, status: FrameStatus) -> FrameReport {
        let residency = self.chunk_residency.metrics();
        let derived = self.derived_cache.usage();
        let physical = self.device.resource_memory();
        let pending = residency.uploads.active_tickets;
        FrameReport {
            status,
            completeness: if pending == 0 {
                FrameCompleteness::Complete
            } else {
                FrameCompleteness::Progressive {
                    pending_chunks: pending,
                }
            },
            degradation: FrameDegradation::streaming_proxy(
                self.mode == RenderMode::Realtime && pending > 0,
            ),
            metrics: FrameMetrics {
                tracked_chunks: residency.tracked_chunks,
                upload_in_flight_bytes: residency.uploads.in_flight_bytes,
                derived_cache_gpu_bytes: derived.gpu_bytes,
                derived_cache_peak_gpu_bytes: derived.peak_gpu_bytes,
                physical_buffer_bytes: physical.buffer_bytes,
                physical_texture_bytes: physical.texture_bytes,
                physical_total_bytes: physical.total_bytes(),
                physical_peak_bytes: physical.peak_bytes,
            },
            needs_another_frame: status == FrameStatus::Skipped
                || pending != 0
                || self
                    .temporal
                    .needs_another_frame(self.tier().temporal_samples()),
            quality_tier: self.tier(),
        }
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

    pub(super) fn rebuild_pool_if_needed(&mut self) -> Result<bool, RenderError> {
        let rebuild = self
            .pool
            .as_ref()
            .is_none_or(|pool| !pool.matches(self.width, self.height));
        if !rebuild {
            return Ok(false);
        }
        let plan = plan_aliases(&self.resources, &self.pass_nodes, &self.order);
        // Old views keep their textures alive. Release bindings first so a
        // resize only reserves the new pool, including a large-to-small resize.
        self.bindings = None;
        self.pool = None;
        self.temporal.reset();
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
        Ok(true)
    }

    fn acquire_surface_frame(
        &mut self,
    ) -> Result<Option<<D::Surface as molgfx_gpu::Surface<D>>::Frame>, RenderError> {
        let Some(surface) = &mut self.surface else {
            return Ok(None);
        };
        match surface.acquire() {
            Ok(frame) => Ok(Some(frame)),
            Err(SurfaceError::Lost | SurfaceError::Outdated) => {
                surface.configure(
                    &self.device,
                    &molgfx_gpu::SurfaceConfig {
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
