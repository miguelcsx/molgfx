//! Capability-gated whole-graph GPU and CPU frame timing.

use super::{Engine, ImageConfig, MotionBlur, RenderMode, TemporalOptions, fit_shadow};
use crate::error::RenderError;
use pdviewx_core::Scene;
use pdviewx_gpu::{
    BufferDesc, BufferUsage, CommandEncoder as _, Device, Queue as _, TextureDesc, TextureUsage,
    TextureViewDesc,
};
use pdviewx_math::Camera;

const QUERY_BYTES: u64 = 2 * std::mem::size_of::<u64>() as u64;

/// One measured frame. GPU time covers the complete scheduled render graph;
/// CPU time covers scene synchronization, graph recording, and submission.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FrameTiming {
    /// Device execution time in nanoseconds.
    pub gpu_ns: u64,
    /// Host frame-construction time in nanoseconds.
    pub cpu_ns: u64,
}

#[derive(Debug)]
pub(crate) struct GpuProfiler<D: Device> {
    queries: D::QuerySet,
    resolve: D::Buffer,
    readback: D::Buffer,
    target: Option<D::Texture>,
    target_view: Option<D::TextureView>,
    target_size: (u32, u32),
    pending_start: Option<u64>,
}

impl<D: Device> GpuProfiler<D> {
    pub(crate) fn new(device: &D) -> Result<Option<Self>, RenderError> {
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
            dimension: pdviewx_gpu::TextureDimension::D2,
            format: pdviewx_gpu::TextureFormat::Rgba8Unorm,
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
    ) -> Result<u64, RenderError> {
        let data = queue
            .read_buffer_async(device, &self.readback, 0, QUERY_BYTES)
            .await?;
        self.decode_timing(&data, queue.timestamp_period())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_timing(&mut self, device: &D, queue: &D::Queue) -> Result<u64, RenderError> {
        let data = queue.read_buffer_blocking(device, &self.readback, 0, QUERY_BYTES)?;
        self.decode_timing(&data, queue.timestamp_period())
    }

    fn decode_timing(&mut self, data: &[u8], timestamp_period: f32) -> Result<u64, RenderError> {
        let Some(start) = read_u64(data, 0) else {
            return Err(pdviewx_gpu::GpuError::DeviceLost.into());
        };
        let Some(end) = read_u64(data, std::mem::size_of::<u64>()) else {
            return Err(pdviewx_gpu::GpuError::DeviceLost.into());
        };
        let previous_start = self.pending_start.replace(start);
        let ticks = timestamp_delta(start, end, previous_start);
        let ticks = u32::try_from(ticks).map_or(u32::MAX, |value| value);
        let seconds = f64::from(ticks) * f64::from(timestamp_period) / 1_000_000_000.0;
        let duration = std::time::Duration::try_from_secs_f64(seconds)
            .map_err(|_| pdviewx_gpu::GpuError::DeviceLost)?;
        Ok(duration_ns(duration))
    }
}

impl<D: Device> Engine<D> {
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
        let (cpu_start, quality) = self.prepare_profile(scene, camera, config)?;
        let Some(mut profiler) = self.profiler.take() else {
            return Err(pdviewx_gpu::GpuError::Capability {
                name: "timestamp queries",
            }
            .into());
        };
        let result = match self.submit_profile(&mut profiler, cpu_start, config, quality) {
            Ok(cpu_ns) => profiler
                .read_timing_async(&self.device, &self.queue)
                .await
                .map(|gpu_ns| FrameTiming { gpu_ns, cpu_ns }),
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
        let (cpu_start, quality) = self.prepare_profile(scene, camera, config)?;
        let Some(mut profiler) = self.profiler.take() else {
            return Err(pdviewx_gpu::GpuError::Capability {
                name: "timestamp queries",
            }
            .into());
        };
        let result = self.profile_with(&mut profiler, cpu_start, config, quality);
        self.profiler = Some(profiler);
        result
    }

    fn prepare_profile(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
    ) -> Result<(std::time::Instant, bool), RenderError> {
        config.validate(self.device.capabilities().max_texture_dim)?;
        let cpu_start = std::time::Instant::now();
        self.width = config.width;
        self.height = config.height;
        let reset = self.prepare_image(scene)?;
        let camera_changed = self.temporal.camera_changed(camera);
        let quality = self.mode == RenderMode::Quality && !camera_changed;
        let optics = self.resolve_optics(scene, camera)?;
        let shadow = fit_shadow(scene, camera, self.resolved_plan.lighting());
        let uniforms = self.temporal.prepare(
            camera,
            &TemporalOptions {
                extent: [self.width, self.height],
                reset: reset || (self.mode == RenderMode::Quality && camera_changed),
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
        Ok((cpu_start, quality))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn profile_with(
        &mut self,
        profiler: &mut GpuProfiler<D>,
        cpu_start: std::time::Instant,
        config: ImageConfig,
        quality: bool,
    ) -> Result<FrameTiming, RenderError> {
        let cpu_ns = self.submit_profile(profiler, cpu_start, config, quality)?;
        Ok(FrameTiming {
            gpu_ns: profiler.read_timing(&self.device, &self.queue)?,
            cpu_ns,
        })
    }

    fn submit_profile(
        &mut self,
        profiler: &mut GpuProfiler<D>,
        cpu_start: std::time::Instant,
        config: ImageConfig,
        quality: bool,
    ) -> Result<u64, RenderError> {
        profiler.ensure_target(&self.device, config)?;
        let Some(target) = &profiler.target_view else {
            return Err(pdviewx_gpu::GpuError::DeviceLost.into());
        };
        let mut encoder = self.device.create_command_encoder();
        self.scene_gpu
            .record_particle_motion(&mut encoder, &self.passes.particle_motion);
        let timestamps_started = self.scene_gpu.record_trajectories(
            &mut encoder,
            &self.passes.trajectory,
            Some(pdviewx_gpu::TimestampWrites {
                queries: &profiler.queries,
                beginning: Some(0),
                end: None,
            }),
        );
        self.scene_gpu
            .record_surface_fields(&mut encoder, &self.passes.surface_field);
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

fn timestamp_delta(current_start: u64, visible_end: u64, previous_start: Option<u64>) -> u64 {
    if visible_end >= current_start {
        visible_end - current_start
    } else {
        previous_start.map_or(0, |start| visible_end.saturating_sub(start))
    }
}

fn duration_ns(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_nanos()).map_or(u64::MAX, |value| value)
}

#[cfg(test)]
#[path = "profiling_tests.rs"]
mod tests;
