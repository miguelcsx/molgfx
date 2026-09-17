//! The wgpu presentation surface.
//!
//! Acquisition failures become typed errors; the engine reconfigures and
//! skips the frame rather than panicking.

use crate::convert;
use crate::device::WgpuDevice;
use molgfx_gpu::{SurfaceConfig, SurfaceError, TextureFormat};

/// The wgpu surface plus its current configuration.
#[derive(Debug)]
pub struct WgpuSurface {
    surface: wgpu::Surface<'static>,
    wgpu_config: wgpu::SurfaceConfiguration,
    config: SurfaceConfig,
    queue_for_present: Option<wgpu::Queue>,
}

impl WgpuSurface {
    pub(crate) fn new(
        surface: wgpu::Surface<'static>,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
    ) -> Self {
        // A bootstrap 1×1 configuration; the engine reconfigures with the
        // real window size before the first frame.
        let wgpu_config = match surface.get_default_config(adapter, 1, 1) {
            Some(config) => config,
            None => wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: wgpu::TextureFormat::Bgra8Unorm,
                color_space: wgpu::SurfaceColorSpace::Auto,
                width: 1,
                height: 1,
                desired_maximum_frame_latency: 2,
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
            },
        };
        surface.configure(device, &wgpu_config);
        let format = match convert::surface_format_back(wgpu_config.format) {
            Some(format) => format,
            None => TextureFormat::Bgra8Unorm,
        };
        Self {
            surface,
            config: SurfaceConfig {
                width: wgpu_config.width,
                height: wgpu_config.height,
                format,
            },
            wgpu_config,
            queue_for_present: None,
        }
    }

    pub(crate) fn attach_queue(&mut self, queue: wgpu::Queue) {
        self.queue_for_present = Some(queue);
    }
}

/// One acquired swapchain frame, presented through the queue.
#[derive(Debug)]
pub struct WgpuFrame {
    texture: wgpu::SurfaceTexture,
    view: wgpu::TextureView,
    queue: Option<wgpu::Queue>,
}

impl molgfx_gpu::SurfaceFrame<WgpuDevice> for WgpuFrame {
    fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    fn present(self) {
        if let Some(queue) = self.queue {
            queue.present(self.texture);
        }
    }
}

impl molgfx_gpu::Surface<WgpuDevice> for WgpuSurface {
    type Frame = WgpuFrame;

    fn configure(&mut self, device: &WgpuDevice, config: &SurfaceConfig) {
        self.wgpu_config.width = config.width.max(1);
        self.wgpu_config.height = config.height.max(1);
        self.surface.configure(&device.device, &self.wgpu_config);
        self.config.width = self.wgpu_config.width;
        self.config.height = self.wgpu_config.height;
    }

    fn config(&self) -> &SurfaceConfig {
        &self.config
    }

    fn acquire(&mut self) -> Result<WgpuFrame, SurfaceError> {
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Err(SurfaceError::Timeout);
            }
            wgpu::CurrentSurfaceTexture::Outdated => return Err(SurfaceError::Outdated),
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Validation => {
                return Err(SurfaceError::Lost);
            }
        };
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        Ok(WgpuFrame {
            texture,
            view,
            queue: self.queue_for_present.clone(),
        })
    }
}
