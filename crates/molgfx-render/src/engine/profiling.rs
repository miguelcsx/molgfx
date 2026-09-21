//! Capability-gated whole-graph GPU and CPU frame timing.

use super::{Engine, ImageConfig, MotionBlur, RenderMode, TemporalOptions};
use crate::ResidencyMetrics;
use crate::error::RenderError;
use molgfx_core::Scene;
use molgfx_gpu::{
    BufferDesc, BufferUsage, CommandEncoder as _, Device, Queue as _, TextureDesc, TextureUsage,
    TextureViewDesc,
};
use molgfx_math::Camera;
// Uses the host monotonic clock on both native and browser targets.
use web_time::Instant;

const QUERY_BYTES: u64 = 2 * std::mem::size_of::<u64>() as u64;

/// One measured frame. GPU time covers the complete scheduled render graph;
/// CPU time covers scene synchronization, graph recording, and submission.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FrameTiming {
    /// Device execution time in nanoseconds.
    pub gpu_ns: u64,
    /// Whether the timestamp query readback resolved this duration.
    ///
    /// A zero duration is evidence only when this is `true`. Backends may
    /// expose timestamp queries while returning an unresolved zero sentinel.
    pub gpu_timing_resolved: bool,
    /// Host frame-construction time in nanoseconds.
    pub cpu_ns: u64,
    /// End-to-end blocking profile duration, including device completion and
    /// timestamp readback. This is the conservative frame-budget metric when
    /// an adapter reports unusable timestamp values.
    pub frame_ns: u64,
    /// Real residency, upload, command and backpressure counters at completion.
    pub residency: ResidencyMetrics,
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
    queries: D::QuerySet,
    resolve: D::Buffer,
    readback: D::Buffer,
    target: Option<D::Texture>,
    target_view: Option<D::TextureView>,
    target_size: (u32, u32),
    target_format: molgfx_gpu::TextureFormat,
    pending_start: Option<u64>,
}

impl<D: Device> GpuProfiler<D> {
    pub(crate) fn new(
        device: &D,
        target_format: molgfx_gpu::TextureFormat,
    ) -> Result<Option<Self>, RenderError> {
        if !device.capabilities().timestamp_queries() {
            return Ok(None);
        }
        Ok(Some(Self {
            queries: device.create_timestamp_query_set(2)?,
            resolve: device.create_buffer(&BufferDesc {
                label: "frame timestamp resolve",
                size: QUERY_BYTES,
                usage: BufferUsage::QUERY_RESOLVE.union(BufferUsage::COPY_SRC),
            })?,
            readback: device.create_buffer(&BufferDesc {
                label: "frame timestamp readback",
                size: QUERY_BYTES,
                usage: BufferUsage::COPY_DST.union(BufferUsage::MAP_READ),
            })?,
            target: None,
            target_view: None,
            target_size: (0, 0),
            target_format,
            pending_start: None,
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
    ) -> Result<DecodedTiming, RenderError> {
        let data = queue
            .read_buffer_async(device, &self.readback, 0, QUERY_BYTES)
            .await?;
        self.decode_timing(&data, queue.timestamp_period())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_timing(&mut self, device: &D, queue: &D::Queue) -> Result<DecodedTiming, RenderError> {
        let data = queue.read_buffer_blocking(device, &self.readback, 0, QUERY_BYTES)?;
        self.decode_timing(&data, queue.timestamp_period())
    }

    fn decode_timing(
        &mut self,
        data: &[u8],
        timestamp_period: f32,
    ) -> Result<DecodedTiming, RenderError> {
        let Some(start) = read_u64(data, 0) else {
            return Err(molgfx_gpu::GpuError::DeviceLost.into());
        };
        let Some(end) = read_u64(data, std::mem::size_of::<u64>()) else {
            return Err(molgfx_gpu::GpuError::DeviceLost.into());
        };
        let previous_start = self.pending_start.replace(start);
        let Some(ticks) = timestamp_delta(start, end, previous_start) else {
            return Ok(DecodedTiming::unresolved());
        };
        let ticks = crate::fallback(u32::try_from(ticks), u32::MAX);
        let seconds = f64::from(ticks) * f64::from(timestamp_period) / 1_000_000_000.0;
        let duration = std::time::Duration::try_from_secs_f64(seconds)
            .map_err(|_| molgfx_gpu::GpuError::DeviceLost)?;
        Ok(DecodedTiming {
            nanoseconds: duration_ns(duration),
            resolved: true,
        })
    }
}

impl<D: Device> Engine<D> {
    /// Measures sustained native throughput with several frames in flight and
    /// one completion wait. The returned durations are per-frame averages;
    /// unlike [`Self::profile_frame`], the end-to-end value does not charge a
    /// blocking buffer map to every frame.
    ///
    /// # Errors
    ///
    /// Returns a capability error when timestamps are unavailable, or a typed
    /// rendering/device error.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn profile_frame_batch(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
        frames: std::num::NonZeroU32,
    ) -> Result<FrameTiming, RenderError> {
        let frame_start = Instant::now();
        let Some(mut profiler) = self.profiler.take() else {
            return Err(molgfx_gpu::GpuError::Capability {
                name: "timestamp queries",
            }
            .into());
        };
        let mut cpu_ns = 0_u64;
        let result = (|| {
            for _ in 0..frames.get() {
                let (cpu_start, quality) = self.prepare_profile(scene, camera, config)?;
                cpu_ns = cpu_ns.saturating_add(self.submit_profile(
                    &mut profiler,
                    cpu_start,
                    config,
                    quality,
                )?);
            }
            let gpu = profiler.read_timing(&self.device, &self.queue)?;
            let count = u64::from(frames.get());
            Ok(FrameTiming {
                gpu_ns: gpu.nanoseconds,
                gpu_timing_resolved: gpu.resolved,
                cpu_ns: cpu_ns / count,
                frame_ns: duration_ns(frame_start.elapsed()) / count,
                residency: self.scene_gpu.residency_metrics(),
            })
        })();
        self.profiler = Some(profiler);
        result
    }

    /// Asynchronously measures one headless frame using timestamp queries.
    ///
    /// # Errors
    ///
    /// Returns a capability error when the adapter exposes no timestamps,
    /// or a typed rendering/device error.
    pub async fn profile_frame_async(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
    ) -> Result<FrameTiming, RenderError> {
        let frame_start = Instant::now();
        let (cpu_start, quality) = self.prepare_profile(scene, camera, config)?;
        let Some(mut profiler) = self.profiler.take() else {
            return Err(molgfx_gpu::GpuError::Capability {
                name: "timestamp queries",
            }
            .into());
        };
        let result = match self.submit_profile(&mut profiler, cpu_start, config, quality) {
            Ok(cpu_ns) => profiler
                .read_timing_async(&self.device, &self.queue)
                .await
                .map(|gpu| FrameTiming {
                    gpu_ns: gpu.nanoseconds,
                    gpu_timing_resolved: gpu.resolved,
                    cpu_ns,
                    frame_ns: duration_ns(frame_start.elapsed()),
                    residency: self.scene_gpu.residency_metrics(),
                }),
            Err(error) => Err(error),
        };
        self.profiler = Some(profiler);
        result
    }

    /// Measures one headless frame using device timestamp queries. Callers
    /// should discard warmup frames before applying percentile gates.
    ///
    /// # Errors
    ///
    /// Returns a capability error when the adapter exposes no timestamps,
    /// or a typed rendering/device error.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn profile_frame(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
    ) -> Result<FrameTiming, RenderError> {
        let frame_start = Instant::now();
        let (cpu_start, quality) = self.prepare_profile(scene, camera, config)?;
        let Some(mut profiler) = self.profiler.take() else {
            return Err(molgfx_gpu::GpuError::Capability {
                name: "timestamp queries",
            }
            .into());
        };
        let result = self.profile_with(&mut profiler, cpu_start, frame_start, config, quality);
        self.profiler = Some(profiler);
        result
    }

    fn prepare_profile(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
    ) -> Result<(Instant, bool), RenderError> {
        config.validate(self.device.capabilities().max_texture_dim)?;
        self.scene_gpu.begin_frame();
        let cpu_start = Instant::now();
        self.width = config.width;
        self.height = config.height;
        let preparation = self.prepare_image(scene)?;
        let identity = scene.cache_identity();
        let scene_reset = self.temporal_scene_identity.replace(identity) != Some(identity);
        let camera_changed = self.temporal.camera_changed(camera);
        let quality = self.mode == RenderMode::Cinematic;
        let optics = self.resolve_optics(scene, camera)?;
        let shadow = self.shadow_bound.fit(
            scene,
            camera,
            self.resolved_plan.lighting(),
            preparation.scene_changed || preparation.rebuild,
        );
        let uniforms = self.temporal.prepare(
            camera,
            &TemporalOptions {
                extent: [self.width, self.height],
                reset: scene_reset
                    || preparation.rebuild
                    || (self.mode == RenderMode::Cinematic && camera_changed),
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
        self.scene_gpu
            .write_frame_uniforms(&self.queue, &uniforms)?;
        Ok((cpu_start, quality))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn profile_with(
        &mut self,
        profiler: &mut GpuProfiler<D>,
        cpu_start: Instant,
        frame_start: Instant,
        config: ImageConfig,
        quality: bool,
    ) -> Result<FrameTiming, RenderError> {
        let cpu_ns = self.submit_profile(profiler, cpu_start, config, quality)?;
        let gpu = profiler.read_timing(&self.device, &self.queue)?;
        Ok(FrameTiming {
            gpu_ns: gpu.nanoseconds,
            gpu_timing_resolved: gpu.resolved,
            cpu_ns,
            frame_ns: duration_ns(frame_start.elapsed()),
            residency: self.scene_gpu.residency_metrics(),
        })
    }

    fn submit_profile(
        &mut self,
        profiler: &mut GpuProfiler<D>,
        cpu_start: Instant,
        config: ImageConfig,
        quality: bool,
    ) -> Result<u64, RenderError> {
        profiler.ensure_target(&self.device, config)?;
        let Some(target) = &profiler.target_view else {
            return Err(molgfx_gpu::GpuError::DeviceLost.into());
        };
        let mut encoder = self.device.create_command_encoder();
        self.passes
            .cull
            .record_attribute_timelines(&self.scene_gpu, &mut encoder);
        self.passes
            .cull
            .record_instance_timelines(&self.scene_gpu, &mut encoder);
        let point_coordinates_changed = self
            .passes
            .cull
            .record_point_timelines(&self.scene_gpu, &mut encoder);
        self.scene_gpu
            .record_particle_motion(&mut encoder, &self.passes.particle_motion);
        let timestamps_started = self.scene_gpu.record_trajectories(
            &mut encoder,
            &self.passes.trajectory,
            Some(molgfx_gpu::TimestampWrites {
                queries: &profiler.queries,
                beginning: Some(0),
                end: None,
            }),
        );
        let paged_coordinates_changed = self
            .passes
            .cull
            .record_paged_trajectories(&self.scene_gpu, &mut encoder);
        self.scene_gpu.record_dynamic_relations(
            &mut encoder,
            &self.passes.relation_resolve,
            timestamps_started || paged_coordinates_changed || point_coordinates_changed,
        );
        self.scene_gpu
            .record_occupancies(&mut encoder, &self.passes.occupancy);
        self.scene_gpu.record_surface_fields(
            &mut encoder,
            &self.passes.surface_field,
            &self.passes.surface_components,
        );
        self.scene_gpu
            .record_quality_hardware(&mut encoder, quality);
        self.record_image(
            &mut encoder,
            target,
            Some(&profiler.queries),
            quality,
            timestamps_started,
        );
        encoder.resolve_query_set(&profiler.queries, 0..2, &profiler.resolve, 0);
        encoder.copy_buffer_to_buffer(&profiler.resolve, 0, &profiler.readback, 0, QUERY_BYTES);
        self.queue.submit(encoder);
        Ok(duration_ns(cpu_start.elapsed()))
    }
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let slice = bytes.get(offset..offset + std::mem::size_of::<u64>())?;
    let mut array = [0; std::mem::size_of::<u64>()];
    array.copy_from_slice(slice);
    Some(u64::from_le_bytes(array))
}

#[derive(Clone, Copy, Debug)]
struct DecodedTiming {
    nanoseconds: u64,
    resolved: bool,
}

impl DecodedTiming {
    const fn unresolved() -> Self {
        Self {
            nanoseconds: 0,
            resolved: false,
        }
    }
}

fn timestamp_delta(
    current_start: u64,
    visible_end: u64,
    previous_start: Option<u64>,
) -> Option<u64> {
    if current_start == 0 && visible_end == 0 {
        return None;
    }
    if visible_end >= current_start {
        return Some(visible_end - current_start);
    }
    previous_start.and_then(|start| visible_end.checked_sub(start))
}

fn duration_ns(duration: std::time::Duration) -> u64 {
    crate::fallback(u64::try_from(duration.as_nanos()), u64::MAX)
}

#[cfg(test)]
#[path = "profiling_tests.rs"]
mod tests;
