//! A mock device: records every command so graph and engine behavior is
//! testable without a GPU.

use molgfx_gpu::{
    BindGroupDesc, BindGroupLayoutDesc, BindingType, BufferDesc, Capabilities, ComputePassDesc,
    ComputePipelineDesc, DeviceDesc, GpuError, Opened, RenderPassDesc, RenderPipelineDesc,
    SamplerDesc, ShaderModuleDesc, ShaderStages, SurfaceConfig, SurfaceError, TextureDesc,
    TextureFormat, TextureViewDesc, WindowTarget,
};
use std::ops::Range;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
/// One buffer-range binding observed by the mock backend.
pub(crate) type MockBufferBinding = (&'static str, u32, u32, u64, u64);
/// Per-stage storage usage carried by a mock bind-group layout.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MockBindGroupLayout {
    storage_counts: [u32; 3],
}
/// Everything the mock observed, shared across handles.
#[derive(Debug)]
pub(crate) struct MockLog {
    /// Physical buffers created as (id, label, byte size).
    pub buffers: Mutex<Vec<(u32, &'static str, u64)>>,
    /// (buffer id, offset, byte length, source pointer) per write.
    pub writes: Mutex<Vec<(u32, u64, usize, usize)>>,
    /// Exact bytes observed by each buffer write, for upload assertions.
    pub write_payloads: Mutex<Vec<Vec<u8>>>,
    /// GPU-to-GPU buffer copies as (source, destination, bytes).
    pub buffer_copies: Mutex<Vec<(u32, u32, u64)>>,
    /// Bound buffer ranges as (group label, binding, buffer, offset, size).
    pub buffer_bindings: Mutex<Vec<MockBufferBinding>>,
    /// Storage-buffer counts per shader stage for every created layout.
    pub bind_group_layouts: Mutex<Vec<(&'static str, [u32; 3])>>,
    /// (texture label, byte length, source pointer) per upload.
    pub texture_writes: Mutex<Vec<(&'static str, usize, usize)>>,
    /// Draw calls: (pipeline set count irrelevant) — records direct draws.
    pub draws: Mutex<Vec<(Range<u32>, Range<u32>)>>,
    /// Indirect draws: (args buffer id, offset).
    pub indirect_draws: Mutex<Vec<(u32, u64)>>,
    /// Compute dispatches.
    pub dispatches: Mutex<Vec<(u32, u32, u32)>>,
    /// Compute passes in command-recording order.
    pub compute_passes: Mutex<Vec<&'static str>>,
    /// Created textures by label.
    pub textures: Mutex<Vec<&'static str>>,
    /// Created texture extents by label.
    pub texture_extents: Mutex<Vec<(&'static str, [u32; 3])>>,
    /// Created texture formats by label.
    pub texture_formats: Mutex<Vec<(&'static str, TextureFormat)>>,
    /// Submissions.
    pub submits: Mutex<u32>,
    /// Greatest submission issued by the mock queue.
    pub submitted_fence: AtomicU64,
    /// Greatest submission explicitly completed by a test.
    pub completed_fence: AtomicU64,
    /// Keeps asynchronous waits pending until a test releases this queue.
    pub hold_fence: std::sync::atomic::AtomicBool,
    /// First fence held when asynchronous completion is paused.
    pub hold_fence_from: AtomicU64,
    /// Source id returned by categorical pick fixtures.
    pub segment_pick_source: Mutex<u32>,
    /// Label returned by categorical pick fixtures.
    pub segment_pick_label: Mutex<u32>,
    /// Local row returned by molecular pick fixtures.
    pub pick_local_row: Mutex<u32>,
    /// Resident page returned by molecular pick fixtures.
    pub pick_resident_page: Mutex<u32>,
    /// BLAS builds recorded by the hardware quality path.
    pub blas_builds: AtomicU32,
    /// TLAS builds recorded by the hardware quality path.
    pub tlas_builds: AtomicU32,
    /// Acceleration structures allocated by the quality path.
    pub acceleration_allocations: AtomicU32,
    /// Makes the next acceleration operation fail as device loss.
    pub fail_ray_query: std::sync::atomic::AtomicBool,
}
impl Default for MockLog {
    fn default() -> Self {
        Self {
            buffers: Mutex::default(),
            writes: Mutex::default(),
            write_payloads: Mutex::default(),
            buffer_copies: Mutex::default(),
            buffer_bindings: Mutex::default(),
            bind_group_layouts: Mutex::default(),
            texture_writes: Mutex::default(),
            draws: Mutex::default(),
            indirect_draws: Mutex::default(),
            dispatches: Mutex::default(),
            compute_passes: Mutex::default(),
            textures: Mutex::default(),
            texture_extents: Mutex::default(),
            texture_formats: Mutex::default(),
            submits: Mutex::default(),
            submitted_fence: AtomicU64::new(0),
            completed_fence: AtomicU64::new(0),
            hold_fence: std::sync::atomic::AtomicBool::new(false),
            hold_fence_from: AtomicU64::new(0),
            segment_pick_source: Mutex::new(u32::MAX),
            segment_pick_label: Mutex::new(0),
            pick_local_row: Mutex::new(0),
            pick_resident_page: Mutex::new(0),
            blas_builds: AtomicU32::new(0),
            tlas_builds: AtomicU32::new(0),
            acceleration_allocations: AtomicU32::new(0),
            fail_ray_query: std::sync::atomic::AtomicBool::new(false),
        }
    }
}
/// The mock device.
#[derive(Debug, Clone)]
pub(crate) struct MockDevice {
    /// The shared observation log.
    pub log: Arc<MockLog>,
    capabilities: Capabilities,
    next_id: Arc<AtomicU32>,
}

impl Default for MockDevice {
    fn default() -> Self {
        Self {
            log: Arc::new(MockLog::default()),
            capabilities: Capabilities {
                flags: molgfx_gpu::CapabilityFlags::empty(),
                min_uniform_buffer_offset_alignment: 256,
                max_storage_buffer_bytes: 1 << 30,
                max_storage_buffers_per_shader_stage: 8,
                max_texture_dim: 16_384,
                max_texture_dim_3d: 2_048,
                r32float: molgfx_gpu::TextureFormatCapabilities {
                    sampled: true,
                    storage_write: true,
                },
                rg32float: molgfx_gpu::TextureFormatCapabilities {
                    sampled: true,
                    storage_write: true,
                },
                rgba32float: molgfx_gpu::TextureFormatCapabilities {
                    sampled: true,
                    storage_write: true,
                },
            },
            next_id: Arc::new(AtomicU32::new(0)),
        }
    }
}

impl MockDevice {
    fn opened() -> Opened<Self> {
        let device = Self::default();
        let queue = MockQueue {
            log: Arc::clone(&device.log),
        };
        let surface = MockSurface {
            script: Vec::new(),
            config: SurfaceConfig {
                width: 64,
                height: 64,
                format: TextureFormat::Bgra8Unorm,
            },
            configures: 0,
        };
        Opened {
            device,
            queue,
            surface: Some(surface),
        }
    }

    pub(crate) fn queue(&self) -> MockQueue {
        MockQueue {
            log: Arc::clone(&self.log),
        }
    }

    pub(crate) fn with_storage_limit(max_storage_buffer_bytes: u64) -> Self {
        let mut device = Self::default();
        device.capabilities.max_storage_buffer_bytes = max_storage_buffer_bytes;
        device
    }

    pub(crate) fn with_ray_query() -> Self {
        let mut device = Self::default();
        device.capabilities.flags |= molgfx_gpu::CapabilityFlags::RAY_QUERY;
        device
    }

    pub(crate) fn with_timestamp_queries() -> Self {
        let mut device = Self::default();
        device.capabilities.flags |= molgfx_gpu::CapabilityFlags::TIMESTAMP_QUERIES;
        device
    }

    pub(crate) fn fail_next_ray_query(&self) {
        self.log.fail_ray_query.store(true, Ordering::Release);
    }

    pub(crate) fn complete_submissions(&self) {
        let submitted = self.log.submitted_fence.load(Ordering::Acquire);
        self.log.completed_fence.store(submitted, Ordering::Release);
    }
}

/// A mock buffer, identified for the log.
#[derive(Debug)]
pub(crate) struct MockBuffer {
    /// Unique id, referenced by the log.
    pub id: u32,
    /// Creation label used by deterministic readback fixtures.
    pub label: &'static str,
    /// Byte capacity used by readback range validation.
    pub size: u64,
    /// Host readability used by readback validation.
    pub usage: molgfx_gpu::BufferUsage,
}

/// A mock timestamp query set.
#[derive(Debug)]
pub(crate) struct MockQuerySet;

/// Mock bottom-level acceleration structure.
#[derive(Debug)]
pub(crate) struct MockBlas;

/// Mock top-level acceleration structure.
#[derive(Debug)]
pub(crate) struct MockTlas {
    instances: Vec<bool>,
}

/// A mock texture; carries its creation label.
#[derive(Debug)]
pub(crate) struct MockTexture(pub &'static str);
/// A mock texture view; carries the source label.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MockView(pub &'static str);

/// The mock encoder: forwards everything to the log.
#[derive(Debug)]
pub(crate) struct MockEncoder {
    log: Arc<MockLog>,
    timestamps: Option<molgfx_gpu::PassTimestampCapture<MockQuerySet>>,
}

/// A mock pass; render and compute share it.
#[derive(Debug)]
pub(crate) struct MockPass<'e> {
    log: &'e Arc<MockLog>,
}

/// The mock queue.
#[derive(Debug)]
pub(crate) struct MockQueue {
    log: Arc<MockLog>,
}

/// A mock surface with scripted acquire results.
#[derive(Debug)]
pub(crate) struct MockSurface {
    /// Pre-programmed outcomes, consumed front to back; empty = success.
    pub script: Vec<Result<(), SurfaceError>>,
    config: SurfaceConfig,
    /// How many times configure ran.
    pub configures: u32,
}

/// A mock acquired frame.
#[derive(Debug)]
pub(crate) struct MockFrame {
    view: MockView,
}

impl molgfx_gpu::SurfaceFrame<MockDevice> for MockFrame {
    fn view(&self) -> &MockView {
        &self.view
    }
    fn present(self) {}
}

impl molgfx_gpu::Surface<MockDevice> for MockSurface {
    type Frame = MockFrame;

    fn configure(&mut self, _device: &MockDevice, config: &SurfaceConfig) {
        self.config = *config;
        self.configures += 1;
    }

    fn config(&self) -> &SurfaceConfig {
        &self.config
    }

    fn acquire(&mut self) -> Result<MockFrame, SurfaceError> {
        let outcome = if self.script.is_empty() {
            Ok(())
        } else {
            self.script.remove(0)
        };
        outcome.map(|()| MockFrame {
            view: MockView("swapchain"),
        })
    }
}

impl molgfx_gpu::Queue<MockDevice> for MockQueue {
    fn write_buffer(&self, buffer: &MockBuffer, offset: u64, data: &[u8]) {
        if let Ok(mut writes) = self.log.writes.lock() {
            writes.push((buffer.id, offset, data.len(), data.as_ptr() as usize));
        }
        if let Ok(mut payloads) = self.log.write_payloads.lock() {
            payloads.push(data.to_vec());
        }
    }

    fn write_texture(&self, texture: &MockTexture, write: &molgfx_gpu::TextureWrite<'_>) {
        if let Ok(mut writes) = self.log.texture_writes.lock() {
            writes.push((texture.0, write.data.len(), write.data.as_ptr() as usize));
        }
    }

    fn submit(&self, _encoder: MockEncoder) {
        if let Ok(mut submits) = self.log.submits.lock() {
            *submits += 1;
        }
    }

    fn submit_tracked(&self, _encoder: MockEncoder) -> molgfx_gpu::FenceValue {
        if let Ok(mut submits) = self.log.submits.lock() {
            *submits += 1;
        }
        let fence = self.log.submitted_fence.fetch_add(1, Ordering::AcqRel) + 1;
        molgfx_gpu::FenceValue(fence)
    }

    fn completed_fence(&self, _device: &MockDevice) -> Result<molgfx_gpu::FenceValue, GpuError> {
        Ok(molgfx_gpu::FenceValue(
            self.log.completed_fence.load(Ordering::Acquire),
        ))
    }

    async fn wait_fence<'a>(
        &'a self,
        device: &'a MockDevice,
        fence: molgfx_gpu::FenceValue,
    ) -> Result<(), GpuError> {
        std::future::poll_fn(|_| {
            if self.log.hold_fence.load(Ordering::Acquire)
                && fence.0 >= self.log.hold_fence_from.load(Ordering::Acquire)
            {
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(())
            }
        })
        .await;
        device.complete_submissions();
        if self.completed_fence(device)? < fence {
            return Err(GpuError::DeviceLost);
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn wait_fence_blocking(
        &self,
        device: &MockDevice,
        fence: molgfx_gpu::FenceValue,
    ) -> Result<(), GpuError> {
        device.complete_submissions();
        if self.completed_fence(device)? < fence {
            return Err(GpuError::DeviceLost);
        }
        Ok(())
    }

    async fn read_buffer_async(
        &self,
        device: &MockDevice,
        buffer: &MockBuffer,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, GpuError> {
        molgfx_gpu::Readback::resolve(&self.readback(device, buffer), offset, size).await
    }

    async fn read_buffer_into_async<'a>(
        &'a self,
        device: &'a MockDevice,
        buffer: &'a MockBuffer,
        offset: u64,
        size: u64,
        output: &'a mut [u8],
    ) -> Result<(), GpuError> {
        molgfx_gpu::Readback::resolve_into(&self.readback(device, buffer), offset, size, output)
            .await
    }

    fn readback(&self, _device: &MockDevice, buffer: &MockBuffer) -> MockReadback {
        MockReadback {
            label: buffer.label,
            size: buffer.size,
            usage: buffer.usage,
            log: Arc::clone(&self.log),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_buffer_blocking(
        &self,
        device: &MockDevice,
        buffer: &MockBuffer,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, GpuError> {
        self.readback(device, buffer).resolve_bytes(offset, size)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_buffer_into_blocking(
        &self,
        device: &MockDevice,
        buffer: &MockBuffer,
        offset: u64,
        size: u64,
        output: &mut [u8],
    ) -> Result<(), GpuError> {
        self.readback(device, buffer)
            .copy_bytes(offset, size, output)
    }

    fn timestamp_period(&self) -> f32 {
        1.0
    }
}

mod device;
mod encoder;
mod readback;
use encoder::fail_mock_ray_query;

pub(crate) use readback::MockReadback;
