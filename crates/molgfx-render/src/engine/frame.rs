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
use crate::graph::{PassContext, ResourceTable};
use molgfx_core::Scene;
use molgfx_gpu::Queue as _;
use molgfx_gpu::{Device, SurfaceFrame as _};
use molgfx_math::Camera;

/// Returns the number of frame submissions currently pending completion.
#[inline]
fn pending_frame_submissions<D: Device>(engine: &Engine<D>) -> u32 {
    u32::from(engine.frame_submission_pending)
}

/// Converts a duration to nanoseconds, saturating instead of panicking on
/// overflow.
#[inline]
fn duration_ns(duration: std::time::Duration) -> u64 {
    duration
        .as_secs()
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::from(duration.subsec_nanos()))
}

impl<D: Device> Engine<D> {
    /// Uploads everything the scene changed since the previous frame.
    ///
    /// Returns whether scene data changed and invalidates temporal convergence
    /// when scene data changed or uploads remain in flight.
    ///
    /// # Errors
    ///
    /// Returns an error when occupancy preparation, scene synchronization, or
    /// chunk residency synchronization fails.
    fn sync_scene(&mut self, scene: &Scene) -> Result<bool, RenderError> {
        self.ensure_occupancy(scene)?;
        self.chunk_residency.poll(&self.device, &self.queue)?;

        let scene_changed = self.scene_gpu.sync(crate::scene_gpu::SceneSync {
            device: &self.device,
            queue: &self.queue,
            scene,
            quality: self.tier() >= QualityTier::Standard,
            detail: self.tier().detail(),
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
        self.chunk_residency.flush(&self.device, &self.queue)?;

        self.derived_frame = self.derived_frame.wrapping_add(1);

        if scene_changed {
            self.temporal.invalidate_convergence();
        }

        Ok(scene_changed)
    }

    /// Updates the cached temporal scene identity.
    ///
    /// Returns `true` when the identity changed. An unchanged scene avoids an
    /// unnecessary write to the cached identity on the steady-state path.
    #[inline]
    fn update_temporal_scene_identity(&mut self, scene: &Scene) -> bool {
        let identity = scene.cache_identity();
        let changed = self.temporal_scene_identity != Some(identity);

        if changed {
            self.temporal_scene_identity = Some(identity);
        }

        changed
    }

    /// Returns whether scene state invalidates temporal accumulation.
    #[inline]
    pub(crate) fn temporal_reset_required(scene_reset: bool, pool_rebuilt: bool) -> bool {
        scene_reset || pool_rebuilt
    }

    /// Renders one frame of the scene to the presentation surface.
    ///
    /// A lost or outdated surface is reconfigured and returns
    /// `FrameStatus::Skipped`; the next frame recovers. Caller-reachable
    /// conditions do not panic.
    ///
    /// # Errors
    ///
    /// Returns an error for unrecoverable GPU errors, scene synchronization
    /// failures, or render-graph resource reconstruction failures.
    pub fn render(&mut self, scene: &Scene, camera: &Camera) -> Result<FrameReport, RenderError> {
        // On native targets the adaptive controller observes this call's CPU
        // duration — synchronization, recording and submission included. The
        // submission itself is asynchronous there, so measuring host time is
        // the honest per-frame cost; the fence poll at the top of the next
        // call deliberately does not sample a second time. On browser targets
        // submission returns immediately, so the controller instead samples
        // submission-to-fence-completion elapsed time in
        // `poll_pending_submission`; measuring this call there would classify
        // queued GPU work as free.
        #[cfg(not(target_arch = "wasm32"))]
        let render_started_at = self.clock_origin.elapsed();

        if self.poll_pending_submission()? {
            // Fence-only skip: the previous submission is still pending.
            // This is not surface/pool work that a retry produces; whether
            // another frame is needed is decided by the report's upload and
            // temporal conditions below.
            #[cfg(not(test))]
            return Ok(self.frame_report(FrameStatus::Skipped, true));
        }

        self.prepare_frame(scene, camera)?;

        let Some(frame) = self.acquire_surface_frame()? else {
            return Ok(self.frame_report(FrameStatus::Skipped, false));
        };

        let mut encoder = self.device.create_command_encoder();

        if !self.record_frame(&mut encoder, scene, frame.view())? {
            return Ok(self.frame_report(FrameStatus::Skipped, false));
        }

        self.submit_frame(encoder);
        self.device.check_errors()?;
        frame.present();

        // CPU encoding/submission cost for the native adaptive loop. This is
        // not device execution time; GPU time needs timestamp queries.
        #[cfg(not(target_arch = "wasm32"))]
        self.adaptive.observe(duration_ns(
            self.clock_origin
                .elapsed()
                .saturating_sub(render_started_at),
        ));

        Ok(self.frame_report(FrameStatus::Presented, false))
    }

    /// Synchronizes CPU/GPU state and prepares per-frame temporal uniforms.
    ///
    /// Scene specializations are settled once here before command recording,
    /// avoiding redundant specialization work during the presented-frame path.
    ///
    /// # Errors
    ///
    /// Returns an error for GPU validation or device failures, scene
    /// synchronization failures, pool reconstruction failures, optics
    /// resolution failures, or frame-uniform upload failures.
    fn prepare_frame(&mut self, scene: &Scene, camera: &Camera) -> Result<(), RenderError> {
        self.adaptive.set_atom_count(scene.atom_count());
        self.sync_quality_tier();
        self.device.check_errors()?;

        self.chunk_residency.begin_epoch();
        self.scene_gpu.begin_frame();

        let scene_changed = self.sync_scene(scene)?;
        let pool_rebuilt = self.rebuild_pool_if_needed()?;

        self.scene_gpu
            .settle_specializations(&self.device, scene, &self.passes);

        let scene_reset = self.update_temporal_scene_identity(scene);
        let cinematic = self.tier() >= QualityTier::Standard;

        let optics = self.resolve_optics(scene, camera)?;
        let shadow =
            self.shadow_bound
                .fit(scene, camera, self.resolved_plan.lighting(), scene_changed);

        let uniforms = self.temporal.prepare(
            camera,
            &TemporalOptions {
                extent: [self.width, self.height],
                reset: Self::temporal_reset_required(scene_reset, pool_rebuilt),
                quality: cinematic,
                publication: self.tier() == QualityTier::High && !self.adaptive.enabled(),
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
        );

        self.scene_gpu.write_frame_uniforms(&self.queue, &uniforms)
    }

    /// Records compute work and render-graph passes into the frame encoder.
    ///
    /// Returns `false` when no transient pool is available.
    ///
    /// `scene` remains part of the existing signature for compatibility.
    /// Specialization settlement is performed once by `prepare_frame` before
    /// this method is reached.
    fn record_frame(
        &mut self,
        encoder: &mut D::CommandEncoder,
        scene: &Scene,
        swapchain: &D::TextureView,
    ) -> Result<bool, RenderError> {
        // Preserve the existing signature without repeating specialization work.
        let _ = scene;

        let cinematic = self.tier() >= QualityTier::Standard;

        self.record_scene_compute(encoder, cinematic, None);

        let Some(pool) = &self.pool else {
            return Ok(false);
        };

        let table = ResourceTable { pool, swapchain };
        let mut failure = None;

        for &index in &self.order {
            let Some(node) = self.pass_nodes.get(index) else {
                continue;
            };

            let mut ctx = PassContext {
                encoder: &mut *encoder,
                device: &self.device,
                target_format: self.target_format,
                failure: &mut failure,
                resources: &table,
                passes: &self.passes,
                bindings: self.bindings.as_ref(),
                scene: &self.scene_gpu,
                timestamps: None,
                temporal_write: self.temporal.write_index(),
                quality: cinematic,
                edge_smoothing: self.edge_smoothing(),
                display_encoding: self.display_encoding(),
            };

            (node.record)(&mut ctx);
        }

        match failure {
            Some(error) => Err(error),
            None => Ok(true),
        }
    }

    /// Submits the recorded frame once and updates submission bookkeeping.
    ///
    /// The submission timestamp is host time when the encoder reached the
    /// queue; the completion timestamp is cleared and stays unset until a
    /// later poll observes the backend fence. That fence fires when submitted
    /// GPU work is done *executing* on the device, strictly later than host
    /// queue-completion, and is never GPU execution time itself.
    fn submit_frame(&mut self, encoder: D::CommandEncoder) {
        self.last_submission_id = self.last_submission_id.wrapping_add(1);
        self.last_submission_timestamp_ns = duration_ns(self.clock_origin.elapsed());
        self.last_completion_timestamp_ns = None;

        self.last_frame_submission = self.queue.submit_tracked(encoder);
        self.frame_submission_pending = true;
        self.submitted_frame_quality = Some(self.effective_quality(
            u32::from(self.tier().temporal_samples()),
            self.temporal.prepared_samples(),
        ));

        // Only the browser adaptive loop samples submission-to-completion
        // elapsed time, so only it needs the submission moment recorded.
        #[cfg(target_arch = "wasm32")]
        {
            self.frame_submitted_at = Some(self.clock_origin.elapsed());
        }
    }

    /// Polls the submission fence and updates completion timing.
    ///
    /// Returns whether the most recent frame submission remains pending.
    ///
    /// A pending-to-complete transition records the host completion
    /// timestamp. That timestamp is host-observation latency, not exact device
    /// completion time: the backend fence callback may have fired earlier, and
    /// the poll that notices it runs on the host clock. GPU execution time is
    /// not reported here at all; it requires timestamp queries and belongs to
    /// the profiling path.
    ///
    /// On browser targets a completed submission also feeds one
    /// submission-to-completion elapsed time to the adaptive controller. The
    /// sample includes host scheduling latency between submit and the fence
    /// callback, so it is a conservative queue-depth signal, not device time.
    /// On native targets this poll is a no-op for the controller: `render`
    /// already measured the frame's full CPU duration before returning, and a
    /// second observation here would double-count the frame.
    ///
    /// # Errors
    ///
    /// Returns an error when the device cannot report the completed fence.
    fn poll_pending_submission(&mut self) -> Result<bool, RenderError> {
        let was_pending = self.frame_submission_pending;
        let completed = self.queue.completed_fence(&self.device)?;

        self.frame_submission_pending = completed < self.last_frame_submission;

        if was_pending && !self.frame_submission_pending {
            self.last_completion_timestamp_ns = Some(duration_ns(self.clock_origin.elapsed()));
            if let Some(mut quality) = self.submitted_frame_quality.take() {
                quality.observe_completion();
                self.last_completed_frame = Some(super::CompletedFrame {
                    submission_id: self.last_submission_id,
                    quality,
                });
            }

            // Only the browser path has an empty controller sample here. On
            // native, `render` observed the complete CPU duration, so a fence
            // observation would double-count this frame's budget.
            #[cfg(target_arch = "wasm32")]
            if let Some(submitted_at) = self.frame_submitted_at.take() {
                self.adaptive.observe(duration_ns(
                    self.clock_origin.elapsed().saturating_sub(submitted_at),
                ));
            }
        }

        Ok(self.frame_submission_pending)
    }

    /// Records scene-level compute passes required before render-graph passes.
    ///
    /// Coordinate-change signals are combined without allocating and drive
    /// dynamic relation resolution exactly once.
    pub(super) fn record_scene_compute(
        &mut self,
        encoder: &mut D::CommandEncoder,
        cinematic: bool,
        timestamps: Option<molgfx_gpu::TimestampWrites<'_, D>>,
    ) -> bool {
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
                .record_trajectories(encoder, &self.passes.trajectory, timestamps);

        let paged_coordinates_changed = self
            .passes
            .cull
            .record_paged_trajectories(&self.scene_gpu, encoder);

        let coordinates_changed =
            structure_coordinates_changed || paged_coordinates_changed || point_coordinates_changed;

        self.scene_gpu.record_dynamic_relations(
            encoder,
            &self.passes.relation_resolve,
            coordinates_changed,
        );

        self.scene_gpu
            .record_occupancies(encoder, self.passes.occupancy.as_ref());

        self.scene_gpu.record_surface_fields(
            encoder,
            &self.passes.surface_field,
            &self.passes.surface_components,
        );

        self.scene_gpu.record_quality_hardware(encoder, cinematic);
        structure_coordinates_changed
    }

    /// Builds the externally visible report for the current frame state.
    ///
    /// The report samples existing counters and resource metrics without
    /// introducing heap allocation in this layer.
    ///
    /// `fence_pending` marks the skip as a poll of a still-pending previous
    /// submission. Such a skip is not surface or pool work that the next
    /// frame recovers, so it does not by itself request another frame:
    /// pending uploads and temporal convergence remain authoritative. A
    /// surface/pool skip (`fence_pending == false`) keeps the existing
    /// retry-next-frame contract.
    fn frame_report(&self, status: FrameStatus, fence_pending: bool) -> FrameReport {
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
                self.mode == RenderMode::Realtime && pending != 0,
            ),
            metrics: FrameMetrics {
                tracked_chunks: residency.tracked_chunks,
                upload_in_flight_bytes: residency.uploads.in_flight_bytes,
                pending_frame_submissions: pending_frame_submissions(self),
                last_submission_id: self.last_submission_id,
                submission_timestamp_ns: self.last_submission_timestamp_ns,
                completion_timestamp_ns: self.last_completion_timestamp_ns,
                derived_cache_gpu_bytes: derived.gpu_bytes,
                derived_cache_peak_gpu_bytes: derived.peak_gpu_bytes,
                physical_buffer_bytes: physical.buffer_bytes,
                physical_texture_bytes: physical.texture_bytes,
                physical_total_bytes: physical.total_bytes(),
                physical_peak_bytes: physical.peak_bytes,
            },
            needs_another_frame: (status == FrameStatus::Skipped && !fence_pending) || pending != 0,
            quality_tier: self.tier(),
            quality: self.effective_quality(
                u32::from(self.tier().temporal_samples()),
                if status == FrameStatus::Presented {
                    self.temporal.prepared_samples()
                } else {
                    0
                },
            ),
            last_completed: self.last_completed_frame,
        }
    }
}
