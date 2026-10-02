//! Off-screen rendering and mapped publication images.
use super::{Engine, QualityTier};
use crate::error::RenderError;
use crate::graph::{PassContext, ResourceTable};
use molgfx_core::Scene;
use molgfx_gpu::{
    BufferDesc, BufferUsage, CommandEncoder as _, Device, FenceValue, Queue as _, TextureDesc,
    TextureFormat, TextureUsage, TextureViewDesc,
};
use molgfx_math::Camera;
#[path = "image/layout.rs"]
mod layout;
pub(super) use layout::ImageLayout;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImagePurpose {
    Publication,
    #[cfg(not(target_arch = "wasm32"))]
    ProgressiveSequence,
}
#[derive(Clone, Copy)]
pub(super) struct ImagePreparation {
    pub(super) scene_changed: bool,
    pub(super) rebuild: bool,
}
/// Requested off-screen image dimensions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImageConfig {
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
}
impl ImageConfig {
    /// Standard 3840×2160 publication output.
    #[must_use]
    pub const fn publication_4k() -> Self {
        Self {
            width: 3840,
            height: 2160,
        }
    }

    pub(super) fn validate(self, max_dimension: u32) -> Result<(), RenderError> {
        if self.width == 0
            || self.height == 0
            || self.width > max_dimension
            || self.height > max_dimension
        {
            return Err(RenderError::InvalidImageSize);
        }
        Ok(())
    }
}

/// Mapped, tightly packed RGBA8 image owned by the caller.
#[derive(Clone, PartialEq, Debug)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major RGBA8 pixels with no row padding.
    pub pixels: Vec<u8>,
    /// Settings and completed exposure observed for these pixels.
    pub quality: super::EffectiveQuality,
}

impl Image {
    /// Encodes the already-tonemapped image as a deterministic RGBA PNG.
    ///
    /// The engine performs scene-linear HDR lighting, exposure, display
    /// grading and sRGB conversion before the readback. This method therefore
    /// serializes publication pixels without applying a second colour curve.
    /// No filesystem or application state is touched.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::ImageEncoding`] if the pixel buffer is malformed
    /// or the PNG encoder rejects the stream.
    pub fn png_bytes(&self) -> Result<Vec<u8>, RenderError> {
        let mut bytes = Vec::new();
        self.write_png(&mut bytes)?;
        Ok(bytes)
    }

    /// Streams deterministic RGBA PNG bytes into a caller-owned writer.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::ImageEncoding`] for malformed pixels or an
    /// encoder/writer failure.
    pub fn write_png(&self, output: impl std::io::Write) -> Result<(), RenderError> {
        self.validate_pixels()?;
        let mut encoder = png::Encoder::new(output, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| RenderError::ImageEncoding {
                summary: error.to_string(),
            })?;
        writer
            .write_image_data(&self.pixels)
            .map_err(|error| RenderError::ImageEncoding {
                summary: error.to_string(),
            })?;
        writer.finish().map_err(|error| RenderError::ImageEncoding {
            summary: error.to_string(),
        })
    }

    fn validate_pixels(&self) -> Result<(), RenderError> {
        let expected = usize::try_from(self.width)
            .ok()
            .and_then(|width| width.checked_mul(usize::try_from(self.height).ok()?))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(RenderError::InvalidImageSize)?;
        if self.pixels.len() != expected {
            return Err(RenderError::ImageEncoding {
                summary: "RGBA8 pixel buffer length does not match image dimensions".to_owned(),
            });
        }
        Ok(())
    }
}

impl<D: Device> Engine<D> {
    /// Asynchronously renders a deterministic off-screen image. Browser
    /// callers use this path so mapped-buffer completion yields to the event
    /// loop.
    ///
    /// # Errors
    ///
    /// Returns a typed device error, graph allocation failure, or
    /// [`RenderError::InvalidImageSize`] for zero or overflowing dimensions.
    pub async fn render_image_async(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
    ) -> Result<Image, RenderError> {
        let pending =
            self.render_image_to_buffer(scene, camera, config, ImagePurpose::Publication)?;
        let mapped = self
            .queue
            .read_buffer_async(&self.device, &pending.buffer, 0, pending.layout.buffer_size)
            .await?;
        pending.resolve(mapped, self.target_format)
    }

    /// Renders one deterministic off-screen frame without opening a window.
    ///
    /// # Errors
    ///
    /// Returns a typed device error, graph allocation failure, or
    /// [`RenderError::InvalidImageSize`] for zero or overflowing dimensions.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_image(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
    ) -> Result<Image, RenderError> {
        let pending =
            self.render_image_to_buffer(scene, camera, config, ImagePurpose::Publication)?;
        let mapped = self.queue.read_buffer_blocking(
            &self.device,
            &pending.buffer,
            0,
            pending.layout.buffer_size,
        )?;
        pending.resolve(mapped, self.target_format)
    }

    pub(super) fn render_image_to_buffer(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
        purpose: ImagePurpose,
    ) -> Result<PendingImage<D>, RenderError> {
        config.validate(self.device.capabilities().max_texture_dim)?;
        let layout = ImageLayout::new(config, 4)?;
        self.width = config.width;
        self.height = config.height;
        let mut exposure = self.prepare_exposure(scene, camera, purpose)?;
        let texture = self.device.create_texture(&TextureDesc {
            label: "off-screen image",
            width: config.width,
            height: config.height,
            depth: 1,
            dimension: molgfx_gpu::TextureDimension::D2,
            format: self.target_format,
            usage: TextureUsage::RENDER_ATTACHMENT.union(TextureUsage::COPY_SRC),
        })?;
        let view = self
            .device
            .create_texture_view(&texture, &TextureViewDesc::default());
        let readback = self.device.create_buffer(&BufferDesc {
            label: "off-screen readback",
            size: layout.buffer_size,
            usage: BufferUsage::COPY_DST.union(BufferUsage::MAP_READ),
        })?;
        let samples = exposure.samples;
        let mut encoder = self.device.create_command_encoder();
        for sample in 0..samples {
            self.prepare_exposure_sample(&mut exposure, sample)?;
            let cinematic = self.tier() >= QualityTier::Standard;
            if sample == 0 {
                self.record_scene_compute(&mut encoder, cinematic, None);
            }
            self.record_image(&mut encoder, &view, None, cinematic, false)?;
            if sample + 1 == samples {
                encoder.copy_texture_to_buffer(
                    &texture,
                    (0, 0),
                    (config.width, config.height),
                    layout.padded_row,
                    0,
                    &readback,
                );
            }
            self.flush_exposure_run(&mut exposure, sample, &mut encoder)?;
        }
        let completion = self.submit_exposure(&exposure, encoder)?;
        let mut quality = self.effective_quality(
            if purpose == ImagePurpose::Publication {
                samples
            } else {
                u32::from(self.tier().temporal_samples())
            },
            self.temporal.prepared_samples(),
        );
        quality.progressive = purpose != ImagePurpose::Publication;
        Ok(PendingImage {
            config,
            layout,
            buffer: readback,
            _texture: texture,
            completion,
            quality,
        })
    }

    /// Prepares the off-screen frame: scene sync, tier publication and pool
    /// rebuild.
    ///
    /// Converged outputs pin maximum detail. Explicit progressive sequences
    /// restore the configured realtime policy and retain compatible history.
    pub(super) fn prepare_image(
        &mut self,
        scene: &Scene,
        purpose: ImagePurpose,
    ) -> Result<ImagePreparation, RenderError> {
        self.ensure_occupancy(scene)?;
        self.device.check_errors()?;
        self.adaptive
            .set_publication(purpose == ImagePurpose::Publication);
        self.adaptive.set_atom_count(scene.atom_count());
        self.sync_quality_tier();
        self.chunk_residency.begin_epoch();
        let scene_changed = self.scene_gpu.sync(crate::scene_gpu::SceneSync {
            device: &self.device,
            queue: &self.queue,
            scene,
            quality: self.tier() >= QualityTier::Standard,
            detail: self.tier().detail(),
            extent: [self.width, self.height],
            ray_query_layout: self.passes.ambient_occlusion.ray_query_layout(),
            derived_cache: &mut self.derived_cache,
            derived_frame: self.derived_frame,
        })?;
        self.chunk_residency.sync_scene(
            &mut self.scene_gpu,
            &self.device,
            &self.queue,
            &mut self.derived_cache,
            self.derived_frame,
        )?;
        self.derived_frame = self.derived_frame.wrapping_add(1);
        let rebuild = self.rebuild_pool_if_needed()?;
        self.scene_gpu
            .settle_specializations(&self.device, scene, &self.passes);
        self.device.check_errors()?;
        Ok(ImagePreparation {
            scene_changed,
            rebuild,
        })
    }

    pub(super) fn record_image(
        &self,
        encoder: &mut D::CommandEncoder,
        target: &D::TextureView,
        queries: Option<&D::QuerySet>,
        quality: bool,
        timestamps_started: bool,
    ) -> Result<(), RenderError> {
        self.record_image_until(encoder, target, queries, quality, timestamps_started, None)
    }

    pub(super) fn record_image_until(
        &self,
        encoder: &mut D::CommandEncoder,
        target: &D::TextureView,
        queries: Option<&D::QuerySet>,
        quality: bool,
        timestamps_started: bool,
        stop_after: Option<crate::graph::ResourceId>,
    ) -> Result<(), RenderError> {
        let Some(pool) = &self.pool else {
            return Ok(());
        };
        let mut failure = None;
        let table = ResourceTable {
            pool,
            swapchain: target,
        };
        for (position, &index) in self.order.iter().enumerate() {
            let Some(node) = self.pass_nodes.get(index) else {
                continue;
            };
            let boundary = position == 0 || position + 1 == self.order.len();
            (node.record)(&mut PassContext {
                encoder,
                device: &self.device,
                target_format: self.target_format,
                failure: &mut failure,
                resources: &table,
                passes: &self.passes,
                bindings: self.bindings.as_ref(),
                scene: &self.scene_gpu,
                timestamps: queries
                    .filter(|_| {
                        boundary && (!timestamps_started || position + 1 == self.order.len())
                    })
                    .map(|queries| molgfx_gpu::TimestampWrites {
                        queries,
                        beginning: (position == 0 && !timestamps_started).then_some(0),
                        end: (position + 1 == self.order.len()).then_some(1),
                    }),
                temporal_write: self.temporal.write_index(),
                quality,
                edge_smoothing: self.edge_smoothing(),
                display_encoding: self.display_encoding(),
            });
            if stop_after.is_some_and(|resource| node.writes.contains(&resource)) {
                break;
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

pub(super) struct PendingImage<D: Device> {
    config: ImageConfig,
    layout: ImageLayout,
    buffer: D::Buffer,
    _texture: D::Texture,
    completion: FenceValue,
    quality: super::EffectiveQuality,
}

impl<D: Device> PendingImage<D> {
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) const fn completion(&self) -> FenceValue {
        self.completion
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn readback(&self) -> (&D::Buffer, u64) {
        (&self.buffer, self.layout.buffer_size)
    }

    pub(super) fn resolve(
        mut self,
        mapped: Vec<u8>,
        format: TextureFormat,
    ) -> Result<Image, RenderError> {
        let _completion = self.completion;
        self.quality.observe_completion();
        let mut pixels = self.layout.unpack(mapped)?;
        if matches!(
            format,
            TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb
        ) {
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
        }
        Ok(Image {
            width: self.config.width,
            height: self.config.height,
            pixels,
            quality: self.quality,
        })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "image_tests.rs"]
mod tests;
