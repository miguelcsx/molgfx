//! Scene-linear HDR readback without a second full-resolution render target.

use super::image::ImageLayout;
use super::image::ImagePurpose;
use super::{Engine, ImageConfig, QualityTier};
use crate::error::RenderError;
use crate::graph::ResourceId;
use crate::passes::{DOF_RESOURCE, HISTORY_A_RESOURCE, HISTORY_B_RESOURCE, MOTION_BLUR_RESOURCE};
use molgfx_core::Scene;
use molgfx_gpu::{BufferDesc, BufferUsage, CommandEncoder as _, Device, Queue as _};
use molgfx_math::Camera;

/// Tightly packed scene-linear RGBA16F pixels owned by the caller.
///
/// `rgba16f` contains row-major RGBA components as little-endian IEEE 754
/// binary16 words. Keeping the native eight-byte texel avoids expanding a 4K
/// readback to twice its size on the CPU. Exposure, bloom, tone mapping,
/// display-gamut conversion, transfer encoding and screen overlays are not
/// baked into these scene-linear values.
#[derive(PartialEq, Debug)]
pub struct HdrImage {
    /// Width in pixels.
    pub(super) width: u32,
    /// Height in pixels.
    pub(super) height: u32,
    /// Row-major RGBA16F bytes with no row padding.
    pub(super) rgba16f: Vec<u8>,
    pub(super) quality: super::EffectiveQuality,
}

impl HdrImage {
    /// Physical settings captured for this completed HDR exposure.
    #[must_use]
    pub const fn quality(&self) -> &super::EffectiveQuality {
        &self.quality
    }

    /// Width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Borrowed, tightly packed scene-linear RGBA16F pixels.
    #[must_use]
    pub fn rgba16f(&self) -> &[u8] {
        &self.rgba16f
    }

    /// Encodes deterministic, uncompressed half-float RGBA `OpenEXR`.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::ImageEncoding`] for malformed pixels or layout
    /// overflow. Use [`Self::write_exr`] to avoid allocating encoded bytes.
    pub fn exr_bytes(&self) -> Result<Vec<u8>, RenderError> {
        super::exr::encode(self)
    }

    /// Streams deterministic, uncompressed `OpenEXR` to a caller-owned sink.
    ///
    /// Only one planar scanline is allocated and reused, so a 4K export does
    /// not duplicate the complete RGBA16F frame in host memory.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::ImageEncoding`] for malformed pixels, layout
    /// overflow, or a sink write failure.
    pub fn write_exr(&self, writer: impl std::io::Write) -> Result<(), RenderError> {
        super::exr::write(self, writer)
    }
}

impl<D: Device> Engine<D> {
    /// Asynchronously renders the resolved scene-linear HDR input that would
    /// otherwise enter presentation tone mapping.
    ///
    /// Browser callers use this path so mapped-buffer completion yields to the
    /// event loop. The capture copies directly from the graph's RGBA16F
    /// resource and does not allocate another full-resolution GPU texture.
    ///
    /// # Errors
    ///
    /// Returns a typed device error, graph allocation failure, or
    /// [`RenderError::InvalidImageSize`] for invalid dimensions.
    pub async fn render_hdr_image_async(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
    ) -> Result<HdrImage, RenderError> {
        let pending = self.render_hdr_image_to_buffer(scene, camera, config)?;
        let mapped = self
            .queue
            .read_buffer_async(&self.device, &pending.buffer, 0, pending.layout.buffer_size)
            .await?;
        pending.resolve(mapped)
    }

    /// Renders the resolved scene-linear HDR input without opening a window.
    ///
    /// The capture copies directly from the graph's RGBA16F resource and does
    /// not allocate another full-resolution GPU texture.
    ///
    /// # Errors
    ///
    /// Returns a typed device error, graph allocation failure, or
    /// [`RenderError::InvalidImageSize`] for invalid dimensions.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_hdr_image(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
    ) -> Result<HdrImage, RenderError> {
        let pending = self.render_hdr_image_to_buffer(scene, camera, config)?;
        let mapped = self.queue.read_buffer_blocking(
            &self.device,
            &pending.buffer,
            0,
            pending.layout.buffer_size,
        )?;
        pending.resolve(mapped)
    }

    fn render_hdr_image_to_buffer(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
    ) -> Result<PendingHdrImage<D>, RenderError> {
        config.validate(self.device.capabilities().max_texture_dim)?;
        let layout = ImageLayout::new(config, 8)?;
        self.width = config.width;
        self.height = config.height;
        let mut exposure = self.prepare_exposure(scene, camera, ImagePurpose::Converged)?;
        let readback = self.device.create_buffer(&BufferDesc {
            label: "scene-linear HDR readback",
            size: layout.buffer_size,
            usage: BufferUsage::COPY_DST.union(BufferUsage::MAP_READ),
        })?;
        let samples = exposure.samples;
        let mut encoder = self.device.create_command_encoder();
        for sample in 0..samples {
            self.prepare_exposure_sample(&mut exposure, sample)?;
            let quality = self.tier() >= QualityTier::Standard;
            if sample == 0 {
                self.record_scene_compute(&mut encoder, quality, None);
            }
            let source = self.scene_linear_resource();
            let Some(pool) = &self.pool else {
                return Err(molgfx_gpu::GpuError::DeviceLost.into());
            };
            let Some(target) = pool.view(source) else {
                return Err(molgfx_gpu::GpuError::DeviceLost.into());
            };
            self.record_image_until(&mut encoder, target, None, quality, false, Some(source))?;
            if sample + 1 == samples {
                let Some(texture) = pool.texture(source) else {
                    return Err(molgfx_gpu::GpuError::DeviceLost.into());
                };
                encoder.copy_texture_to_buffer(
                    texture,
                    (0, 0),
                    (config.width, config.height),
                    layout.padded_row,
                    0,
                    &readback,
                );
            }
            self.flush_exposure_run(&mut exposure, sample, &mut encoder)?;
        }
        self.submit_exposure(&exposure, encoder)?;
        Ok(PendingHdrImage {
            config,
            layout,
            buffer: readback,
            quality: self.effective_quality(samples, self.temporal.prepared_samples()),
        })
    }

    fn scene_linear_resource(&self) -> ResourceId {
        if self.resolved_plan.motion_blur().is_some() {
            MOTION_BLUR_RESOURCE
        } else if self.resolved_plan.depth_of_field().is_some() {
            DOF_RESOURCE
        } else if self.temporal.write_index() == 0 {
            HISTORY_A_RESOURCE
        } else {
            HISTORY_B_RESOURCE
        }
    }
}

struct PendingHdrImage<D: Device> {
    config: ImageConfig,
    layout: ImageLayout,
    buffer: D::Buffer,
    quality: super::EffectiveQuality,
}

impl<D: Device> PendingHdrImage<D> {
    fn resolve(mut self, mapped: Vec<u8>) -> Result<HdrImage, RenderError> {
        self.quality.observe_completion();
        Ok(HdrImage {
            width: self.config.width,
            height: self.config.height,
            rgba16f: self.layout.unpack(mapped)?,
            quality: self.quality,
        })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "hdr_image_tests.rs"]
mod tests;
