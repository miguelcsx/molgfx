//! A mock device: records every command so graph and engine behavior is
//! testable without a GPU.

use pdviewx_gpu::{
    BindGroupDesc, BindGroupLayoutDesc, BufferDesc, Capabilities, ComputePassDesc,
    ComputePipelineDesc, DeviceDesc, GpuError, Opened, RenderPassDesc, RenderPipelineDesc,
    SamplerDesc, ShaderModuleDesc, SurfaceConfig, SurfaceError, TextureDesc, TextureFormat,
    TextureViewDesc, WindowTarget,
};
use std::ops::Range;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Everything the mock observed, shared across handles.
#[derive(Debug)]
pub struct MockLog {
    /// (buffer id, offset, byte length, source pointer) per write.
    pub writes: Mutex<Vec<(u32, u64, usize, usize)>>,
    /// (texture label, byte length, source pointer) per upload.
    pub texture_writes: Mutex<Vec<(&'static str, usize, usize)>>,
    /// Draw calls: (pipeline set count irrelevant) — records direct draws.
    pub draws: Mutex<Vec<(Range<u32>, Range<u32>)>>,
    /// Indirect draws: (args buffer id, offset).
    pub indirect_draws: Mutex<Vec<(u32, u64)>>,
    /// Compute dispatches.
    pub dispatches: Mutex<Vec<(u32, u32, u32)>>,
    /// Created textures by label.
    pub textures: Mutex<Vec<&'static str>>,
    /// Created texture extents by label.
    pub texture_extents: Mutex<Vec<(&'static str, [u32; 3])>>,
    /// Submissions.
    pub submits: Mutex<u32>,
    /// Source id returned by categorical pick fixtures.
    pub segment_pick_source: Mutex<u32>,
    /// Label returned by categorical pick fixtures.
    pub segment_pick_label: Mutex<u32>,
}

impl Default for MockLog {
    fn default() -> Self {
        Self {
            writes: Mutex::default(),
            texture_writes: Mutex::default(),
            draws: Mutex::default(),
            indirect_draws: Mutex::default(),
            dispatches: Mutex::default(),
            textures: Mutex::default(),
            texture_extents: Mutex::default(),
            submits: Mutex::default(),
            segment_pick_source: Mutex::new(u32::MAX),
            segment_pick_label: Mutex::new(0),
        }
    }
}

/// The mock device.
#[derive(Debug, Clone)]
pub struct MockDevice {
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
                flags: pdviewx_gpu::CapabilityFlags::empty(),
                max_storage_buffer_bytes: 1 << 30,
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
}

/// A mock buffer, identified for the log.
#[derive(Debug)]
pub struct MockBuffer {
    /// Unique id, referenced by the log.
    pub id: u32,
    /// Creation label used by deterministic readback fixtures.
    pub label: &'static str,
}

/// A mock timestamp query set.
#[derive(Debug)]
pub struct MockQuerySet;

/// A mock texture; carries its creation label.
#[derive(Debug)]
pub struct MockTexture(pub &'static str);
/// A mock texture view; carries the source label.
#[derive(Debug, PartialEq, Eq)]
pub struct MockView(pub &'static str);

/// The mock encoder: forwards everything to the log.
#[derive(Debug)]
pub struct MockEncoder {
    log: Arc<MockLog>,
}

/// A mock pass; render and compute share it.
#[derive(Debug)]
pub struct MockPass<'e> {
    log: &'e Arc<MockLog>,
}

/// The mock queue.
#[derive(Debug)]
pub struct MockQueue {
    log: Arc<MockLog>,
}

/// A mock surface with scripted acquire results.
#[derive(Debug)]
pub struct MockSurface {
    /// Pre-programmed outcomes, consumed front to back; empty = success.
    pub script: Vec<Result<(), SurfaceError>>,
    config: SurfaceConfig,
    /// How many times configure ran.
    pub configures: u32,
}

/// A mock acquired frame.
#[derive(Debug)]
pub struct MockFrame {
    view: MockView,
}

impl pdviewx_gpu::SurfaceFrame<MockDevice> for MockFrame {
    fn view(&self) -> &MockView {
        &self.view
    }
    fn present(self) {}
}

impl pdviewx_gpu::Surface<MockDevice> for MockSurface {
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

impl pdviewx_gpu::Queue<MockDevice> for MockQueue {
    fn write_buffer(&self, buffer: &MockBuffer, offset: u64, data: &[u8]) {
        if let Ok(mut writes) = self.log.writes.lock() {
            writes.push((buffer.id, offset, data.len(), data.as_ptr() as usize));
        }
    }

    fn write_texture(&self, texture: &MockTexture, write: &pdviewx_gpu::TextureWrite<'_>) {
        if let Ok(mut writes) = self.log.texture_writes.lock() {
            writes.push((texture.0, write.data.len(), write.data.as_ptr() as usize));
        }
    }

    fn submit(&self, _encoder: MockEncoder) {
        if let Ok(mut submits) = self.log.submits.lock() {
            *submits += 1;
        }
    }

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
    if label == "segment volume pick readback" {
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

impl pdviewx_gpu::CommandEncoder<MockDevice> for MockEncoder {
    type RenderPass<'e> = MockPass<'e>;
    type ComputePass<'e> = MockPass<'e>;

    fn begin_render_pass<'e>(&'e mut self, _desc: &RenderPassDesc<'_, MockDevice>) -> MockPass<'e> {
        MockPass { log: &self.log }
    }

    fn begin_compute_pass<'e>(
        &'e mut self,
        _desc: &ComputePassDesc<'_, MockDevice>,
    ) -> MockPass<'e> {
        MockPass { log: &self.log }
    }

    fn copy_buffer_to_buffer(
        &mut self,
        _s: &MockBuffer,
        _so: u64,
        _d: &MockBuffer,
        _do_: u64,
        _n: u64,
    ) {
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
}

impl pdviewx_gpu::RenderPassEncoder<MockDevice> for MockPass<'_> {
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

impl pdviewx_gpu::ComputePassEncoder<MockDevice> for MockPass<'_> {
    fn set_pipeline(&mut self, _pipeline: &u32) {}
    fn set_bind_group(&mut self, _index: u32, _group: &u32, _offsets: &[u32]) {}
    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        if let Ok(mut dispatches) = self.log.dispatches.lock() {
            dispatches.push((x, y, z));
        }
    }
}

impl pdviewx_gpu::Device for MockDevice {
    type Buffer = MockBuffer;
    type Texture = MockTexture;
    type TextureView = MockView;
    type Sampler = ();
    type ShaderModule = ();
    type BindGroupLayout = ();
    type BindGroup = u32;
    type Pipeline = u32;
    type QuerySet = MockQuerySet;
    type CommandEncoder = MockEncoder;
    type Queue = MockQueue;
    type Surface = MockSurface;

    async fn open_async(
        _desc: &DeviceDesc,
        _window: Option<WindowTarget>,
    ) -> Result<Opened<Self>, GpuError> {
        Ok(Self::opened())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_blocking(
        _desc: &DeviceDesc,
        _window: Option<WindowTarget>,
    ) -> Result<Opened<Self>, GpuError> {
        Ok(Self::opened())
    }

    fn create_buffer(&self, desc: &BufferDesc) -> Result<MockBuffer, GpuError> {
        Ok(MockBuffer {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            label: desc.label,
        })
    }

    fn create_texture(&self, desc: &TextureDesc) -> Result<MockTexture, GpuError> {
        if let Ok(mut textures) = self.log.textures.lock() {
            textures.push(desc.label);
        }
        if let Ok(mut extents) = self.log.texture_extents.lock() {
            extents.push((desc.label, [desc.width, desc.height, desc.depth]));
        }
        Ok(MockTexture(desc.label))
    }

    fn create_texture_view(&self, texture: &MockTexture, _desc: &TextureViewDesc) -> MockView {
        MockView(texture.0)
    }

    fn create_sampler(&self, _desc: &SamplerDesc) {}

    fn create_shader_module(&self, _desc: &ShaderModuleDesc<'_>) -> Result<(), GpuError> {
        Ok(())
    }

    fn create_bind_group_layout(&self, _desc: &BindGroupLayoutDesc<'_>) {}

    fn create_bind_group(&self, _desc: &BindGroupDesc<'_, Self>) -> u32 {
        0
    }

    fn create_render_pipeline(
        &self,
        _desc: &RenderPipelineDesc<'_, Self>,
    ) -> Result<u32, GpuError> {
        Ok(0)
    }

    fn create_compute_pipeline(
        &self,
        _desc: &ComputePipelineDesc<'_, Self>,
    ) -> Result<u32, GpuError> {
        Ok(0)
    }

    fn create_command_encoder(&self) -> MockEncoder {
        MockEncoder {
            log: Arc::clone(&self.log),
        }
    }

    fn create_timestamp_query_set(&self, _count: u32) -> Result<MockQuerySet, GpuError> {
        if self.capabilities.timestamp_queries() {
            Ok(MockQuerySet)
        } else {
            Err(GpuError::Capability {
                name: "timestamp queries",
            })
        }
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }
}
