//! Rendering into textures owned by an embedding application.
//!
//! This is the one place the renderer's public API names the graphics
//! backend: a host that already owns a wgpu device lends it here, and gets
//! its textures drawn into without a CPU copy.

use super::Renderer;
use super::engine_config::engine_config;
use crate::{Error, RenderProfile, Scene};
use molgfx_gpu::{FenceValue, HostDeviceDesc, TextureFormat};
use molgfx_wgpu::{HostContext, WgpuDevice};

/// A texture the renderer may draw into, checked on creation.
pub type HostTarget = molgfx_render::HostTarget<WgpuDevice>;

/// One host-target frame: its report and the fence that completes with it.
pub use molgfx_render::HostFrame;

/// The 8-bit non-sRGB format the renderer writes into host targets. The
/// shaders encode for display themselves, so an sRGB texture is drawn
/// through its linear view (list that view format when creating it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostOutputFormat {
    /// `Rgba8Unorm`, with `Rgba8UnormSrgb` accepted as the texture format.
    Rgba8,
    /// `Bgra8Unorm`, with `Bgra8UnormSrgb` accepted as the texture format.
    Bgra8,
}

impl HostOutputFormat {
    const fn texture_format(self) -> TextureFormat {
        match self {
            Self::Rgba8 => TextureFormat::Rgba8Unorm,
            Self::Bgra8 => TextureFormat::Bgra8Unorm,
        }
    }
}

impl Renderer {
    /// Builds a renderer on the host's wgpu adapter, device and queue.
    ///
    /// The device must satisfy `molgfx_wgpu::host_requirements`; every
    /// unmet requirement is listed in the error. The host's error callbacks
    /// are left in place, and submissions go to the host's queue, so work
    /// the host submits afterwards is ordered after the renderer's.
    ///
    /// # Errors
    ///
    /// Returns a typed error for unmet requirements or pipeline failures.
    pub fn from_host(
        profile: RenderProfile,
        context: HostContext,
        format: HostOutputFormat,
    ) -> Result<Self, Error> {
        let config = engine_config(profile, None);
        let inner = molgfx_render::Engine::from_host(
            &config,
            context,
            &HostDeviceDesc::default(),
            format.texture_format(),
        )?;
        Ok(Self {
            inner,
            profile,
            pick_source_id: None,
        })
    }

    /// Checks a host texture (render attachment, 2-D, one layer, one mip,
    /// not multisampled, in the output format family) for use as a target.
    ///
    /// # Errors
    ///
    /// Returns every problem found with the texture.
    pub fn host_target(&self, texture: &molgfx_wgpu::wgpu::Texture) -> Result<HostTarget, Error> {
        Ok(self.inner.import_target(texture)?)
    }

    /// Creates a texture the renderer owns, for hosts that only present it.
    /// Resize by creating a new target and dropping the old one.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero or oversized extent, or allocation failure.
    pub fn create_host_target(&self, width: u32, height: u32) -> Result<HostTarget, Error> {
        Ok(self.inner.create_target(width, height)?)
    }

    /// Renders one realtime frame into `target`. Call again while the
    /// report says another frame is needed. Picking then answers for this
    /// target's extent.
    ///
    /// # Errors
    ///
    /// Returns a typed scene, device or synchronization error.
    pub fn render_to_host(
        &mut self,
        scene: &Scene,
        camera: &molgfx_math::Camera,
        target: &HostTarget,
    ) -> Result<HostFrame, Error> {
        let frame = self
            .inner
            .render_to_target(scene.resolved(), camera, target)?;
        self.pick_source_id = Some(scene.resolved().cache_identity());
        Ok(frame)
    }

    /// Renders one converged exposure into `target`: the pixels
    /// `render_image_with_camera` would return, left on the GPU.
    ///
    /// # Errors
    ///
    /// Returns a typed scene, device or synchronization error.
    pub fn render_to_host_converged(
        &mut self,
        scene: &Scene,
        camera: &molgfx_math::Camera,
        target: &HostTarget,
    ) -> Result<HostFrame, Error> {
        let frame = self
            .inner
            .render_to_target_converged(scene.resolved(), camera, target)?;
        self.pick_source_id = Some(scene.resolved().cache_identity());
        Ok(frame)
    }

    /// Whether the GPU has finished the submission `fence` names.
    ///
    /// # Errors
    ///
    /// Returns an error when the device is lost.
    pub fn host_fence_completed(&self, fence: FenceValue) -> Result<bool, Error> {
        Ok(self.inner.fence_completed(fence)?)
    }

    /// Awaits `fence` without blocking a thread.
    ///
    /// # Errors
    ///
    /// Returns an error when the device is lost.
    pub async fn wait_host_fence(&self, fence: FenceValue) -> Result<(), Error> {
        Ok(self.inner.wait_fence(fence).await?)
    }

    /// Blocks until `fence` completes.
    ///
    /// # Errors
    ///
    /// Returns an error when the device is lost.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn wait_host_fence_blocking(&self, fence: FenceValue) -> Result<(), Error> {
        Ok(self.inner.wait_fence_blocking(fence)?)
    }
}
