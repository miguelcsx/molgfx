//! The device trait: the root of the hardware abstraction.
//!
//! `Device` carries every backend resource as an associated type, so the
//! renderer is generic over it and pays no dynamic dispatch on the hot path.
//! Behavior traits (`Queue`, `CommandEncoder`, `Surface`) take the device as
//! a type parameter to name those resource types in their signatures.

use crate::capabilities::Capabilities;
use crate::descriptors::{
    BindGroupDesc, BindGroupLayoutDesc, BufferDesc, ComputePipelineDesc, RenderPipelineDesc,
    SamplerDesc, ShaderModuleDesc, TextureDesc, TextureViewDesc,
};
use crate::encoder::CommandEncoder;
use crate::error::GpuError;
use crate::queue::Queue;
use crate::surface::Surface;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::future::Future;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

/// Anything a presentation surface can be created from. Embedders hand the
/// engine their window behind this trait; the engine never names a
/// windowing toolkit.
///
/// Native window handles may cross worker threads. Browser canvas handles are
/// bound to the JavaScript main thread, so requiring `Send + Sync` there would
/// reject the platform's real WebGPU resources.
#[cfg(not(target_arch = "wasm32"))]
pub trait WindowSource: HasWindowHandle + HasDisplayHandle + std::fmt::Debug + Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: HasWindowHandle + HasDisplayHandle + std::fmt::Debug + Send + Sync> WindowSource for T {}

/// Browser presentation source, intentionally confined to the JavaScript
/// thread that owns its canvas.
#[cfg(target_arch = "wasm32")]
pub trait WindowSource: HasWindowHandle + HasDisplayHandle + std::fmt::Debug {}
#[cfg(target_arch = "wasm32")]
impl<T: HasWindowHandle + HasDisplayHandle + std::fmt::Debug> WindowSource for T {}

/// A shared native window handle, alive for as long as its surface.
#[cfg(not(target_arch = "wasm32"))]
pub type WindowTarget = Arc<dyn WindowSource>;

/// A browser canvas supplied and owned by the host page.
#[cfg(target_arch = "wasm32")]
pub type WindowTarget = web_sys::HtmlCanvasElement;

/// Which adapter class to prefer when several are present.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PowerPreference {
    /// The fastest available adapter; the default for a rendering engine.
    #[default]
    HighPerformance,
    /// The most efficient adapter.
    LowPower,
}

/// Options for opening a device.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeviceDesc {
    /// Adapter preference.
    pub power: PowerPreference,
}

/// The result of opening a device: the device, its queue, and a surface
/// when a window was supplied.
#[derive(Debug)]
pub struct Opened<D: Device> {
    /// The device.
    pub device: D,
    /// Its submission queue.
    pub queue: D::Queue,
    /// The presentation surface, when opened against a window.
    pub surface: Option<D::Surface>,
}

/// A GPU device: resource creation and capability report.
///
/// Everything created here is created at load and reused; the trait offers
/// no per-frame conveniences by design.
pub trait Device: Sized + 'static {
    /// GPU buffer.
    type Buffer: std::fmt::Debug;
    /// GPU texture.
    type Texture: std::fmt::Debug;
    /// View over a texture, bindable or attachable.
    type TextureView: std::fmt::Debug;
    /// Texture sampler.
    type Sampler: std::fmt::Debug;
    /// Compiled shader module.
    type ShaderModule: std::fmt::Debug;
    /// Bind-group layout.
    type BindGroupLayout: std::fmt::Debug;
    /// Bind group.
    type BindGroup: std::fmt::Debug;
    /// Render or compute pipeline.
    type Pipeline: std::fmt::Debug;
    /// Timestamp or occlusion query storage.
    type QuerySet: std::fmt::Debug;
    /// Command encoder.
    type CommandEncoder: CommandEncoder<Self>;
    /// Submission queue.
    type Queue: Queue<Self>;
    /// Presentation surface.
    type Surface: Surface<Self>;

    /// Selects an adapter and asynchronously opens a device, with a surface
    /// when a window is supplied.
    ///
    /// # Errors
    ///
    /// No compatible adapter, or device creation failed.
    fn open_async(
        desc: &DeviceDesc,
        window: Option<WindowTarget>,
    ) -> impl Future<Output = Result<Opened<Self>, GpuError>>;

    /// Native convenience for callers that do not already run an async
    /// executor. Browser builds expose only asynchronous device opening.
    ///
    /// # Errors
    ///
    /// No compatible adapter, or device creation failed.
    #[cfg(not(target_arch = "wasm32"))]
    fn open_blocking(
        desc: &DeviceDesc,
        window: Option<WindowTarget>,
    ) -> Result<Opened<Self>, GpuError>;

    /// Creates a buffer.
    ///
    /// # Errors
    ///
    /// The buffer exceeded device limits.
    fn create_buffer(&self, desc: &BufferDesc) -> Result<Self::Buffer, GpuError>;

    /// Creates a 2-D texture.
    ///
    /// # Errors
    ///
    /// The texture exceeded device limits.
    fn create_texture(&self, desc: &TextureDesc) -> Result<Self::Texture, GpuError>;

    /// Creates a view over a texture.
    fn create_texture_view(
        &self,
        texture: &Self::Texture,
        desc: &TextureViewDesc,
    ) -> Self::TextureView;

    /// Creates a sampler.
    fn create_sampler(&self, desc: &SamplerDesc) -> Self::Sampler;

    /// Compiles a WGSL shader module.
    ///
    /// # Errors
    ///
    /// Compilation failed; the error carries the backend's detail.
    fn create_shader_module(
        &self,
        desc: &ShaderModuleDesc<'_>,
    ) -> Result<Self::ShaderModule, GpuError>;

    /// Creates a bind-group layout.
    fn create_bind_group_layout(&self, desc: &BindGroupLayoutDesc<'_>) -> Self::BindGroupLayout;

    /// Creates a bind group over a layout.
    fn create_bind_group(&self, desc: &BindGroupDesc<'_, Self>) -> Self::BindGroup;

    /// Creates a render pipeline. Created at load, cached by the caller.
    ///
    /// # Errors
    ///
    /// Pipeline creation failed (most often shader/interface mismatch).
    fn create_render_pipeline(
        &self,
        desc: &RenderPipelineDesc<'_, Self>,
    ) -> Result<Self::Pipeline, GpuError>;

    /// Creates a compute pipeline.
    ///
    /// # Errors
    ///
    /// Pipeline creation failed.
    fn create_compute_pipeline(
        &self,
        desc: &ComputePipelineDesc<'_, Self>,
    ) -> Result<Self::Pipeline, GpuError>;

    /// Creates a command encoder for one frame or task.
    fn create_command_encoder(&self) -> Self::CommandEncoder;

    /// Creates a timestamp query set when the capability is available.
    ///
    /// # Errors
    ///
    /// Returns a capability error when timestamp queries are unavailable.
    fn create_timestamp_query_set(&self, count: u32) -> Result<Self::QuerySet, GpuError>;

    /// The opened device's capability report.
    fn capabilities(&self) -> &Capabilities;
}
