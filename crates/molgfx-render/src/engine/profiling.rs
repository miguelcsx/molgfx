//! Completed-output latency and capability-gated whole-graph GPU timing.

use super::image::ImagePurpose;
use super::{Engine, ImageConfig, QualityTier};
use crate::ResidencyMetrics;
use crate::error::RenderError;
use molgfx_core::Scene;
use molgfx_gpu::{
    CommandEncoder as _, Device, Queue as _, TextureDesc, TextureUsage, TextureViewDesc,
};
use molgfx_math::Camera;
// Uses the host monotonic clock on both native and browser targets.
use web_time::Instant;

/// One measured frame. GPU time covers the complete scheduled render graph;
/// CPU time covers scene synchronization, graph recording, and submission.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FrameTiming {
    /// GPU interval or an explicit timestamp availability reason.
    pub gpu_timing: super::GpuTiming,
    /// Host frame-construction time in nanoseconds.
    pub cpu_ns: u64,
    /// Disjoint elapsed host intervals; waiting is not CPU utilization.
    pub cpu_stages: super::CpuStages,
    /// End-to-end blocking profile duration, including device completion and
    /// timestamp readback. This is the conservative frame-budget metric when
    /// an adapter reports unusable timestamp values.
    pub frame_ns: u64,
    /// Real residency, upload, command and backpressure counters at completion.
    pub residency: ResidencyMetrics,
    /// Selected settings and exposure completion observed with this timing.
    pub quality: super::EffectiveQuality,
    /// Actual pass capture coverage, including explicit bounded overflow.
    pub pass_coverage: super::PassTimingCoverage,
}

impl FrameTiming {
    /// Flat cumulative residency counters at measurement completion.
    #[must_use]
    pub fn residency_counters(self) -> crate::ResidencyCounters {
        self.residency.counters()
    }
}

#[derive(Debug)]
pub(crate) struct GpuProfiler<D: Device> {
    passes: super::pass_profiling::PassProfiler<D>,
    completion: molgfx_gpu::FenceValue,
    cpu_stages: super::CpuStages,
    target: Option<D::Texture>,
    target_view: Option<D::TextureView>,
    target_size: (u32, u32),
    target_format: molgfx_gpu::TextureFormat,
}

impl<D: Device> GpuProfiler<D> {
    pub(crate) fn new(
        device: &D,
        target_format: molgfx_gpu::TextureFormat,
    ) -> Result<Option<Self>, RenderError> {
        let passes = super::pass_profiling::PassProfiler::new(device)?;
        Ok(Some(Self {
            passes,
            completion: molgfx_gpu::FenceValue::default(),
            cpu_stages: super::CpuStages::default(),
            target: None,
            target_view: None,
            target_size: (0, 0),
            target_format,
        }))
    }

    fn ensure_target(&mut self, device: &D, config: ImageConfig) -> Result<(), RenderError> {
        if self.target_size == (config.width, config.height) {
            return Ok(());
        }
        let target = device.create_texture(&TextureDesc {
            label: "profiling target",
            width: config.width,
            height: config.height,
            depth: 1,
            dimension: molgfx_gpu::TextureDimension::D2,
            format: self.target_format,
            usage: TextureUsage::RENDER_ATTACHMENT,
        })?;
        self.target_view = Some(device.create_texture_view(&target, &TextureViewDesc::default()));
        self.target = Some(target);
        self.target_size = (config.width, config.height);
        Ok(())
    }

    async fn read_timing_async(
        &mut self,
        device: &D,
        queue: &D::Queue,
    ) -> Result<super::GpuTiming, RenderError> {
        let wait_started = Instant::now();
        queue.wait_fence(device, self.completion).await?;
        self.cpu_stages.completion_wait_ns = self
            .cpu_stages
            .completion_wait_ns
            .saturating_add(duration_ns(wait_started.elapsed()));
        let read_started = Instant::now();
        if let Some(completion) = self.passes.submit_resolve(device, queue) {
            // Retain the second submission before suspension, just like the
            // exposure fence, so cancellation cannot make scratch reusable.
            self.completion = completion;
            queue.wait_fence(device, completion).await?;
        }
        let timing = self.passes.read_async(device, queue).await;
        self.cpu_stages.timestamp_readback_ns = duration_ns(read_started.elapsed());
        timing
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_timing(
        &mut self,
        device: &D,
        queue: &D::Queue,
    ) -> Result<super::GpuTiming, RenderError> {
        let wait_started = Instant::now();
        queue.wait_fence_blocking(device, self.completion)?;
        self.cpu_stages.completion_wait_ns = self
            .cpu_stages
            .completion_wait_ns
            .saturating_add(duration_ns(wait_started.elapsed()));
        let read_started = Instant::now();
        if let Some(completion) = self.passes.submit_resolve(device, queue) {
            self.completion = completion;
            queue.wait_fence_blocking(device, completion)?;
        }
        let timing = self.passes.read(device, queue);
        self.cpu_stages.timestamp_readback_ns = duration_ns(read_started.elapsed());
        timing
    }

    #[cfg(test)]
    fn decode_timing(data: &[u8], period: f32) -> Result<super::GpuTiming, RenderError> {
        let (Some(start), Some(end)) = (read_u64(data, 0), read_u64(data, 8)) else {
            return Ok(super::GpuTiming::Malformed);
        };
        super::GpuTiming::from_timestamps(start, end, period)
    }
}

impl<D: Device> Engine<D> {
    /// Actual passes from the latest completed profile; borrowed until the next profile.
    #[must_use]
    pub fn last_pass_timings(&self) -> &[super::PassTiming] {
        self.profiler
            .as_ref()
            .map_or(&[], |profiler| profiler.passes.timings())
    }
    /// Asynchronously measures one headless frame using timestamp queries.
    ///
    /// Dropping the future retains its storage and completion fence. The next
    /// profile waits before reusing the target or timestamp capture.
    ///
    /// # Errors
    ///
    /// Returns a typed rendering/device error. Missing timestamps are reported
    /// in `gpu_timing` without preventing completed-output measurement.
    pub async fn profile_frame_async(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
    ) -> Result<FrameTiming, RenderError> {
        let frame_start = Instant::now();
        // A cancelled profile still owns a submission. Do not overwrite its
        // queries, readback storage or target until that submission completes.
        let previous_wait_started = Instant::now();
        self.wait_profile_completion_async().await?;
        let previous_wait_ns = duration_ns(previous_wait_started.elapsed());
        let (cpu_start, quality) = self.prepare_profile(scene, camera, config)?;
        let Some(mut profiler) = self.profiler.take() else {
            return Err(molgfx_gpu::GpuError::DeviceLost.into());
        };
        profiler.cpu_stages = super::CpuStages {
            completion_wait_ns: previous_wait_ns,
            ..super::CpuStages::default()
        };
        let submitted = self.submit_profile(&mut profiler, cpu_start, config, quality);
        // Only synchronous recording borrows the profiler out of the engine.
        // Every suspension point keeps persistent storage in its owner.
        self.profiler = Some(profiler);
        let cpu_ns = submitted?;
        let Some(profiler) = &mut self.profiler else {
            return Err(molgfx_gpu::GpuError::DeviceLost.into());
        };
        let gpu = profiler
            .read_timing_async(&self.device, &self.queue)
            .await?;
        let cpu_stages = profiler.cpu_stages;
        let pass_coverage = profiler.passes.coverage();
        Ok(FrameTiming {
            gpu_timing: gpu,
            cpu_ns,
            cpu_stages,
            frame_ns: duration_ns(frame_start.elapsed()),
            residency: self.scene_gpu.residency_metrics(),
            quality: self.profile_quality(),
            pass_coverage,
        })
    }

    /// Measures one headless frame using device timestamp queries. Callers
    /// should discard warmup frames before applying percentile gates.
    ///
    /// # Errors
    ///
    /// Returns a typed rendering/device error. Missing timestamps are reported
    /// in `gpu_timing` without preventing completed-output measurement.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn profile_frame(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
    ) -> Result<FrameTiming, RenderError> {
        let frame_start = Instant::now();
        let previous_wait_started = Instant::now();
        if let Some(profiler) = &self.profiler
            && self.queue.completed_fence(&self.device)? < profiler.completion
        {
            self.queue
                .wait_fence_blocking(&self.device, profiler.completion)?;
        }
        let previous_wait_ns = duration_ns(previous_wait_started.elapsed());
        let (cpu_start, quality) = self.prepare_profile(scene, camera, config)?;
        let Some(mut profiler) = self.profiler.take() else {
            return Err(molgfx_gpu::GpuError::DeviceLost.into());
        };
        profiler.cpu_stages = super::CpuStages {
            completion_wait_ns: previous_wait_ns,
            ..super::CpuStages::default()
        };
        let result = self.profile_with(&mut profiler, cpu_start, frame_start, config, quality);
        self.profiler = Some(profiler);
        result
    }

    async fn wait_profile_completion_async(&self) -> Result<(), RenderError> {
        if let Some(profiler) = &self.profiler
            && self.queue.completed_fence(&self.device)? < profiler.completion
        {
            self.queue
                .wait_fence(&self.device, profiler.completion)
                .await?;
        }
        Ok(())
    }

    fn prepare_profile(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
    ) -> Result<(Instant, super::exposure::Exposure), RenderError> {
        config.validate(self.device.capabilities().max_texture_dim)?;
        let cpu_start = Instant::now();
        self.width = config.width;
        self.height = config.height;
        let exposure = self.prepare_exposure(scene, camera, ImagePurpose::Publication)?;
        Ok((cpu_start, exposure))
    }

    fn profile_quality(&self) -> super::EffectiveQuality {
        let mut quality = self.effective_quality(
            self.tier().image_samples(),
            self.temporal.prepared_samples(),
        );
        quality.observe_completion();
        quality
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn profile_with(
        &mut self,
        profiler: &mut GpuProfiler<D>,
        cpu_start: Instant,
        frame_start: Instant,
        config: ImageConfig,
        quality: super::exposure::Exposure,
    ) -> Result<FrameTiming, RenderError> {
        let cpu_ns = self.submit_profile(profiler, cpu_start, config, quality)?;
        let gpu = profiler.read_timing(&self.device, &self.queue)?;
        Ok(FrameTiming {
            gpu_timing: gpu,
            cpu_ns,
            cpu_stages: profiler.cpu_stages,
            frame_ns: duration_ns(frame_start.elapsed()),
            residency: self.scene_gpu.residency_metrics(),
            quality: self.profile_quality(),
            pass_coverage: profiler.passes.coverage(),
        })
    }

    fn submit_profile(
        &mut self,
        profiler: &mut GpuProfiler<D>,
        cpu_start: Instant,
        config: ImageConfig,
        mut exposure: super::exposure::Exposure,
    ) -> Result<u64, RenderError> {
        profiler.cpu_stages.preparation_ns = profiler
            .cpu_stages
            .preparation_ns
            .saturating_add(duration_ns(cpu_start.elapsed()));
        let recording_started = Instant::now();
        profiler.ensure_target(&self.device, config)?;
        let Some(target) = &profiler.target_view else {
            return Err(molgfx_gpu::GpuError::DeviceLost.into());
        };
        let mut encoder = self.device.create_command_encoder();
        profiler.passes.attach(&mut encoder)?;
        for sample in 0..exposure.samples {
            encoder.set_timestamp_sample(Some(sample));
            if let Err(error) = self.prepare_exposure_sample(&mut exposure, sample) {
                profiler.passes.discard(&mut encoder)?;
                return Err(error);
            }
            let quality = self.tier() >= QualityTier::Standard;
            if sample == 0 {
                self.record_scene_compute(&mut encoder, quality, None);
            }
            self.record_image(&mut encoder, target, None, quality, false)?;
        }
        profiler.passes.detach(&mut encoder)?;
        profiler.cpu_stages.recording_ns = profiler
            .cpu_stages
            .recording_ns
            .saturating_add(duration_ns(recording_started.elapsed()));
        let submission_started = Instant::now();
        profiler.completion = self.submit_exposure(&exposure, encoder)?;
        profiler.cpu_stages.submission_ns = profiler
            .cpu_stages
            .submission_ns
            .saturating_add(duration_ns(submission_started.elapsed()));
        Ok(duration_ns(cpu_start.elapsed()))
    }
}

#[cfg(test)]
fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let slice = bytes.get(offset..offset + std::mem::size_of::<u64>())?;
    let mut array = [0; std::mem::size_of::<u64>()];
    array.copy_from_slice(slice);
    Some(u64::from_le_bytes(array))
}

fn duration_ns(duration: std::time::Duration) -> u64 {
    crate::fallback(u64::try_from(duration.as_nanos()), u64::MAX)
}

#[cfg(test)]
#[path = "profiling_tests.rs"]
mod tests;
