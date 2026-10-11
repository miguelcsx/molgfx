//! Rendering into textures a host application owns, with no readback.
//!
//! A host that embeds the engine in its own window system lends the engine
//! a device ([`Engine::from_host`]) and then names a texture per frame. The
//! engine records one submission on the shared queue and returns a fence;
//! the host orders its own work after it on the same queue, or waits on the
//! fence. Nothing is copied to the CPU.
//!
//! One engine renders any number of targets, one after another. They share
//! the engine's transient pool, temporal history and pick buffers, so
//! alternating between targets of different sizes reallocates the pool and
//! restarts accumulation, and picking answers for the most recently
//! rendered target.

use super::image::{ImageConfig, ImagePurpose};
use super::{Engine, EngineConfig, FrameReport, FrameStatus};
use crate::error::RenderError;
use molgfx_core::Scene;
use molgfx_gpu::{
    FenceValue, HostDevice, HostDeviceDesc, ImportedTarget, Queue, TextureDesc, TextureDimension,
    TextureFormat, TextureUsage,
};
use molgfx_math::Camera;

/// A texture the engine may render into: a host texture it has checked, or
/// one the engine created for the host.
#[derive(Debug)]
pub struct HostTarget<D: HostDevice> {
    view: D::TextureView,
    info: ImportedTarget,
    external: D::ExternalTexture,
}

impl<D: HostDevice> HostTarget<D> {
    /// Pixel width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.info.width
    }

    /// Pixel height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.info.height
    }

    /// The format the engine writes, always a non-sRGB 8-bit format: the
    /// shaders apply the display transfer function themselves.
    #[must_use]
    pub const fn render_format(&self) -> TextureFormat {
        self.info.render_format
    }

    /// The format of the underlying texture, which may be the sRGB sibling
    /// of [`Self::render_format`].
    #[must_use]
    pub const fn texture_format(&self) -> TextureFormat {
        self.info.texture_format
    }

    /// A handle to the underlying texture, for the host to sample or copy.
    #[must_use]
    pub const fn texture(&self) -> &D::ExternalTexture {
        &self.external
    }
}

/// One host-target frame: the usual report plus the submission's fence.
#[derive(Clone, Copy, Debug)]
pub struct HostFrame {
    /// What the frame drew and how complete it is.
    pub report: FrameReport,
    /// Completes when the GPU has finished the frame. Hosts that submit
    /// their own work after it on the same queue need not wait; others pass
    /// it to the engine's fence methods or ignore it.
    pub fence: FenceValue,
}

impl<D: HostDevice> Engine<D> {
    /// Builds an engine on a device the host already owns. `output_format`
    /// is the 8-bit non-sRGB format every target will be written in
    /// (`Rgba8Unorm` or `Bgra8Unorm`).
    ///
    /// The engine takes no surface and, unless the description asks, leaves
    /// the host's device error handlers alone.
    ///
    /// # Errors
    ///
    /// The context lacks a required feature or limit, the format is not a
    /// supported output format, or pipeline construction failed.
    pub fn from_host(
        config: &EngineConfig,
        context: D::Context,
        desc: &HostDeviceDesc,
        output_format: TextureFormat,
    ) -> Result<Self, RenderError> {
        if !output_format.is_host_output() {
            return Err(molgfx_gpu::GpuError::HostTarget {
                problems: vec![format!(
                    "{output_format:?} is not a supported output format; use Rgba8Unorm or Bgra8Unorm"
                )],
            }
            .into());
        }
        let opened = D::adopt(context, desc)?;
        Self::from_opened_with(config, opened, Some(output_format))
    }

    /// Checks a host texture for use as a target. For an sRGB texture the
    /// host must have listed the linear sibling in its `view_formats`.
    ///
    /// # Errors
    ///
    /// The texture is not a single-layer 2-D render attachment in the
    /// engine's format family; every problem is listed.
    pub fn import_target(
        &self,
        texture: &D::ExternalTexture,
    ) -> Result<HostTarget<D>, RenderError> {
        let (view, info) = self.device.import_target(texture, self.target_format)?;
        Ok(HostTarget {
            view,
            info,
            external: texture.clone(),
        })
    }

    /// Creates a target the engine owns, for hosts that only want a texture
    /// to present. Its usages are render attachment, texture binding and
    /// copy source. To resize, create a new target and drop the old one.
    ///
    /// # Errors
    ///
    /// The size is zero or beyond the device limit, or allocation failed.
    pub fn create_target(&self, width: u32, height: u32) -> Result<HostTarget<D>, RenderError> {
        ImageConfig { width, height }.validate(self.device.capabilities().max_texture_dim)?;
        let texture = self.device.create_texture(&TextureDesc {
            label: "host target",
            width,
            height,
            depth: 1,
            dimension: TextureDimension::D2,
            format: self.target_format,
            usage: TextureUsage::RENDER_ATTACHMENT
                .union(TextureUsage::TEXTURE_BINDING)
                .union(TextureUsage::COPY_SRC),
        })?;
        let external = self.device.export_texture(&texture);
        self.import_target(&external)
    }

    /// Renders one realtime frame into `target`: temporal accumulation and
    /// the adaptive quality loop apply, as on a window. Call again while the
    /// report says `needs_another_frame`.
    ///
    /// Unlike a window frame this never skips for a pending previous
    /// submission; pacing is the host's, using the returned fence.
    ///
    /// # Errors
    ///
    /// Scene synchronization, recording or device failures.
    pub fn render_to_target(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        target: &HostTarget<D>,
    ) -> Result<HostFrame, RenderError> {
        // The adaptive loop needs a frame-time sample per frame. A window frame
        // feeds it in `render`; without this a host target never left the
        // initial `Reduced` tier, so a host got reduced detail for good.
        #[cfg(not(target_arch = "wasm32"))]
        let started_at = self.clock_origin.elapsed();
        self.poll_pending_submission()?;
        self.follow_target(target);
        self.prepare_frame(scene, camera)?;
        let mut encoder = self.device.create_command_encoder();
        if !self.record_frame(&mut encoder, scene, &target.view)? {
            return Ok(HostFrame {
                report: self.frame_report(FrameStatus::Skipped),
                fence: self.last_frame_submission,
            });
        }
        self.submit_frame(encoder);
        self.device.check_errors()?;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let elapsed = self.clock_origin.elapsed().saturating_sub(started_at);
            let nanoseconds = u64::try_from(elapsed.as_nanos())
                .into_iter()
                .fold(u64::MAX, |_, value| value);
            self.adaptive.observe(nanoseconds);
        }
        Ok(HostFrame {
            report: self.frame_report(FrameStatus::Presented),
            fence: self.last_frame_submission,
        })
    }

    /// Renders one converged exposure into `target`: the same pixels
    /// [`Self::render_image`] returns, left on the GPU.
    ///
    /// # Errors
    ///
    /// Scene synchronization, recording or device failures.
    pub fn render_to_target_converged(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        target: &HostTarget<D>,
    ) -> Result<HostFrame, RenderError> {
        self.poll_pending_submission()?;
        let config = ImageConfig {
            width: target.width(),
            height: target.height(),
        };
        let (fence, quality) = self.render_exposure(
            scene,
            camera,
            config,
            ImagePurpose::Converged,
            &target.view,
            None,
        )?;
        self.device.check_errors()?;
        let mut report = self.frame_report(FrameStatus::Presented);
        report.quality = quality;
        Ok(HostFrame { report, fence })
    }

    /// Whether the GPU has finished the submission `fence` names.
    ///
    /// # Errors
    ///
    /// The device was lost.
    pub fn fence_completed(&self, fence: FenceValue) -> Result<bool, RenderError> {
        Ok(self.queue.completed_fence(&self.device)? >= fence)
    }

    /// Awaits `fence` without blocking a thread; usable on every target.
    ///
    /// # Errors
    ///
    /// The device was lost.
    pub async fn wait_fence(&self, fence: FenceValue) -> Result<(), RenderError> {
        Ok(self.queue.wait_fence(&self.device, fence).await?)
    }

    /// Blocks until `fence` completes. Native targets only.
    ///
    /// # Errors
    ///
    /// The device was lost.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    pub fn wait_fence_blocking(&self, fence: FenceValue) -> Result<(), RenderError> {
        Ok(self.queue.wait_fence_blocking(&self.device, fence)?)
    }

    /// Makes the engine's frame extent follow the target, restarting
    /// temporal accumulation when it changes.
    fn follow_target(&mut self, target: &HostTarget<D>) {
        if self.width != target.width() || self.height != target.height() {
            self.resize(target.width(), target.height());
        }
    }
}
