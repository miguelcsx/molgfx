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
                max_storage_buffer_bytes: 1 << 30,
                max_storage_buffers_per_shader_stage: 8,
                max_texture_dim: 16_384,
                max_texture_dim_3d: 2_048,
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

    // The trait declares `fn read_buffer_async(..) -> impl Future`, so the
    // `async` keyword is the signature, not a suspension point.
    #[allow(clippy::unused_async_trait_impl)]
    async fn read_buffer_async(
        &self,
        _device: &MockDevice,
        buffer: &MockBuffer,
        _offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, GpuError> {
        mock_readback(size, buffer.label, &self.log)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_buffer_blocking(
        &self,
        _device: &MockDevice,
        buffer: &MockBuffer,
        _offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, GpuError> {
        mock_readback(size, buffer.label, &self.log)
    }

    fn timestamp_period(&self) -> f32 {
        1.0
    }
}

fn mock_readback(size: u64, label: &'static str, log: &Arc<MockLog>) -> Result<Vec<u8>, GpuError> {
    let length = usize::try_from(size).map_err(|_| GpuError::DeviceLost)?;
    let mut bytes = vec![0; length];
    if label == "local row pick readback" {
        let row = log.pick_local_row.lock().map_or(u32::MAX, |row| *row);
        if let Some(word) = bytes.get_mut(..4) {
            word.copy_from_slice(&row.to_le_bytes());
        }
    } else if label == "resident page pick readback" {
        let page = log.pick_resident_page.lock().map_or(u32::MAX, |page| *page);
        if let Some(word) = bytes.get_mut(..4) {
            word.copy_from_slice(&page.to_le_bytes());
        }
    } else if label == "segment volume pick readback" {
        let source = log
            .segment_pick_source
            .lock()
            .map_or(u32::MAX, |source| *source);
        if let Some(word) = bytes.get_mut(..4) {
            word.copy_from_slice(&source.to_le_bytes());
        }
    } else if label == "segment label pick readback" {
        let label = log.segment_pick_label.lock().map_or(0, |label| *label);
        if let Some(word) = bytes.get_mut(..4) {
            word.copy_from_slice(&label.to_le_bytes());
        }
    }
    Ok(bytes)
}

impl molgfx_gpu::CommandEncoder<MockDevice> for MockEncoder {
    type RenderPass<'e> = MockPass<'e>;
    type ComputePass<'e> = MockPass<'e>;

    fn begin_render_pass<'e>(&'e mut self, _desc: &RenderPassDesc<'_, MockDevice>) -> MockPass<'e> {
        MockPass { log: &self.log }
    }

    fn begin_compute_pass<'e>(
        &'e mut self,
        desc: &ComputePassDesc<'_, MockDevice>,
    ) -> MockPass<'e> {
        if let Ok(mut passes) = self.log.compute_passes.lock() {
            passes.push(desc.label);
        }
        MockPass { log: &self.log }
    }

    fn copy_buffer_to_buffer(
        &mut self,
        source: &MockBuffer,
        _so: u64,
        destination: &MockBuffer,
        _do_: u64,
        bytes: u64,
    ) {
        if let Ok(mut copies) = self.log.buffer_copies.lock() {
            copies.push((source.id, destination.id, bytes));
        }
    }

    fn copy_texture_to_buffer(
        &mut self,
        _src: &MockTexture,
        _origin: (u32, u32),
        _size: (u32, u32),
        _bpr: u32,
        _dst: &MockBuffer,
    ) {
    }

    fn resolve_query_set(
        &mut self,
        _queries: &MockQuerySet,
        _range: Range<u32>,
        _dst: &MockBuffer,
        _offset: u64,
    ) {
    }

    fn build_blas(
        &mut self,
        _desc: &molgfx_gpu::BlasBuildDesc<'_, MockDevice>,
    ) -> Result<(), GpuError> {
        fail_mock_ray_query(&self.log)?;
        self.log.blas_builds.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn build_tlas(&mut self, _tlas: &MockTlas) -> Result<(), GpuError> {
        fail_mock_ray_query(&self.log)?;
        self.log.tlas_builds.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

fn fail_mock_ray_query(log: &MockLog) -> Result<(), GpuError> {
    if log.fail_ray_query.swap(false, Ordering::AcqRel) {
        Err(GpuError::DeviceLost)
    } else {
        Ok(())
    }
}

impl molgfx_gpu::RenderPassEncoder<MockDevice> for MockPass<'_> {
    fn set_pipeline(&mut self, _pipeline: &u32) {}
    fn set_bind_group(&mut self, _index: u32, _group: &u32, _offsets: &[u32]) {}
    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        if let Ok(mut draws) = self.log.draws.lock() {
            draws.push((vertices, instances));
        }
    }
    fn draw_indirect(&mut self, args: &MockBuffer, offset: u64) {
        if let Ok(mut indirect) = self.log.indirect_draws.lock() {
            indirect.push((args.id, offset));
        }
    }
}

impl molgfx_gpu::ComputePassEncoder<MockDevice> for MockPass<'_> {
    fn set_pipeline(&mut self, _pipeline: &u32) {}
    fn set_bind_group(&mut self, _index: u32, _group: &u32, _offsets: &[u32]) {}
    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        if let Ok(mut dispatches) = self.log.dispatches.lock() {
            dispatches.push((x, y, z));
        }
    }
}

mod device;
