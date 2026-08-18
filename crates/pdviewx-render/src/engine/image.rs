//! Off-screen rendering and mapped publication images.

use super::{Engine, MotionBlur, RenderMode, TemporalOptions, fit_shadow};
use crate::error::RenderError;
use crate::graph::{PassContext, ResourceTable, TransientPool, plan_aliases};
use crate::passes::FrameBindings;
use pdviewx_core::Scene;
use pdviewx_gpu::{
    BufferDesc, BufferUsage, CommandEncoder as _, Device, Queue as _, TextureDesc, TextureFormat,
    TextureUsage, TextureViewDesc,
};
use pdviewx_math::Camera;

const REALTIME_IMAGE_SAMPLES: u32 = 16;
const QUALITY_IMAGE_SAMPLES: u32 = 64;

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
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major RGBA8 pixels with no row padding.
    pub pixels: Vec<u8>,
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
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, self.width, self.height);
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
        drop(writer);
        Ok(bytes)
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
        let pending = self.render_image_to_buffer(scene, camera, config)?;
        let mapped = self
            .queue
            .read_buffer_async(&self.device, &pending.buffer, 0, pending.layout.buffer_size)
            .await?;
        Ok(pending.resolve(&mapped, self.target_format))
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
        let pending = self.render_image_to_buffer(scene, camera, config)?;
        let mapped = self.queue.read_buffer_blocking(
            &self.device,
            &pending.buffer,
            0,
            pending.layout.buffer_size,
        )?;
        Ok(pending.resolve(&mapped, self.target_format))
    }

    fn render_image_to_buffer(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        config: ImageConfig,
    ) -> Result<PendingImage<D>, RenderError> {
        config.validate(self.device.capabilities().max_texture_dim)?;
        let layout = ImageLayout::new(config)?;
        self.width = config.width;
        self.height = config.height;
        let reset = self.prepare_image(scene)?;
        self.temporal.reset();
        let optics = self.resolve_optics(scene, camera)?;
        let texture = self.device.create_texture(&TextureDesc {
            label: "off-screen image",
            width: config.width,
            height: config.height,
            depth: 1,
            dimension: pdviewx_gpu::TextureDimension::D2,
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
        let samples = match self.mode {
            RenderMode::Realtime => REALTIME_IMAGE_SAMPLES,
            RenderMode::Quality => QUALITY_IMAGE_SAMPLES,
        };
        let shadow = fit_shadow(scene, camera, self.resolved_plan.lighting());
        for sample in 0..samples {
            let camera_changed = self.temporal.camera_changed(camera);
            let quality = self.mode == RenderMode::Quality && !camera_changed;
            let uniforms = self.temporal.prepare(
                camera,
                &TemporalOptions {
                    extent: [self.width, self.height],
                    reset: reset && sample == 0,
                    quality,
                    publication: true,
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
            let mut encoder = self.device.create_command_encoder();
            self.scene_gpu
                .record_particle_motion(&mut encoder, &self.passes.particle_motion);
            self.scene_gpu
                .record_trajectories(&mut encoder, &self.passes.trajectory, None);
            self.scene_gpu
                .record_surface_fields(&mut encoder, &self.passes.surface_field);
            self.record_image(&mut encoder, &view, None, quality, false);
            if sample + 1 == samples {
                encoder.copy_texture_to_buffer(
                    &texture,
                    (0, 0),
                    (config.width, config.height),
                    layout.padded_row,
                    &readback,
                );
            }
            self.queue.submit(encoder);
        }
        Ok(PendingImage {
            config,
            layout,
            buffer: readback,
            _texture: texture,
        })
    }

    pub(super) fn prepare_image(&mut self, scene: &Scene) -> Result<bool, RenderError> {
        let scene_changed = self.scene_gpu.sync(&self.device, &self.queue, scene)?;
        let rebuild = self
            .pool
            .as_ref()
            .is_none_or(|pool| !pool.matches(self.width, self.height));
        if rebuild {
            let plan = plan_aliases(&self.resources, &self.pass_nodes, &self.order);
            self.pool = Some(TransientPool::build(
                &self.device,
                &self.resources,
                plan,
                self.width,
                self.height,
            )?);
            self.bindings = self
                .pool
                .as_ref()
                .and_then(|pool| FrameBindings::new(&self.device, pool, &self.passes));
        }
        Ok(scene_changed || rebuild)
    }

    pub(super) fn record_image(
        &self,
        encoder: &mut D::CommandEncoder,
        target: &D::TextureView,
        queries: Option<&D::QuerySet>,
        quality: bool,
        timestamps_started: bool,
    ) {
        let Some(pool) = &self.pool else {
            return;
        };
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
                resources: &table,
                passes: &self.passes,
                bindings: self.bindings.as_ref(),
                scene: &self.scene_gpu,
                timestamps: queries
                    .filter(|_| {
                        boundary && (!timestamps_started || position + 1 == self.order.len())
                    })
                    .map(|queries| pdviewx_gpu::TimestampWrites {
                        queries,
                        beginning: (position == 0 && !timestamps_started).then_some(0),
                        end: (position + 1 == self.order.len()).then_some(1),
                    }),
                temporal_write: self.temporal.write_index(),
                quality,
                depth_of_field: self.resolved_plan.depth_of_field().is_some(),
                motion_blur: self.resolved_plan.motion_blur().is_some(),
                display_encoding: self.display_encoding(),
            });
        }
    }
}

struct PendingImage<D: Device> {
    config: ImageConfig,
    layout: ImageLayout,
    buffer: D::Buffer,
    _texture: D::Texture,
}

impl<D: Device> PendingImage<D> {
    fn resolve(self, mapped: &[u8], format: TextureFormat) -> Image {
        Image {
            width: self.config.width,
            height: self.config.height,
            pixels: self.layout.unpack(mapped, format),
        }
    }
}

struct ImageLayout {
    row: usize,
    padded_row: u32,
    buffer_size: u64,
    height: usize,
}

impl ImageLayout {
    fn new(config: ImageConfig) -> Result<Self, RenderError> {
        let row = config
            .width
            .checked_mul(4)
            .ok_or(RenderError::InvalidImageSize)?;
        let padded_row = row
            .checked_add(255)
            .map(|value| value & !255)
            .ok_or(RenderError::InvalidImageSize)?;
        let buffer_size = u64::from(padded_row)
            .checked_mul(u64::from(config.height))
            .ok_or(RenderError::InvalidImageSize)?;
        if config.width == 0 || config.height == 0 {
            return Err(RenderError::InvalidImageSize);
        }
        Ok(Self {
            row: row as usize,
            padded_row,
            buffer_size,
            height: config.height as usize,
        })
    }

    fn unpack(&self, mapped: &[u8], format: TextureFormat) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(self.row * self.height);
        for source in mapped
            .chunks_exact(self.padded_row as usize)
            .take(self.height)
        {
            pixels.extend_from_slice(&source[..self.row.min(source.len())]);
        }
        if matches!(
            format,
            TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb
        ) {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }
        pixels
    }
}

#[cfg(test)]
mod tests {
    use super::Image;

    #[test]
    fn publication_png_round_trips_rgba_pixels() {
        let image = Image {
            width: 2,
            height: 1,
            pixels: vec![12, 34, 56, 255, 200, 180, 160, 128],
        };
        let bytes = match image.png_bytes() {
            Ok(bytes) => bytes,
            Err(error) => panic!("PNG encoding succeeds: {error}"),
        };
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = match decoder.read_info() {
            Ok(reader) => reader,
            Err(error) => panic!("PNG header is readable: {error}"),
        };
        let Some(size) = reader.output_buffer_size() else {
            panic!("PNG decoder reports an output buffer")
        };
        let mut pixels = vec![0; size];
        let output = match reader.next_frame(&mut pixels) {
            Ok(output) => output,
            Err(error) => panic!("PNG pixels are readable: {error}"),
        };
        assert_eq!(&pixels[..output.buffer_size()], image.pixels.as_slice());
    }

    #[test]
    fn malformed_publication_image_is_rejected_before_encoding() {
        let image = Image {
            width: 2,
            height: 1,
            pixels: vec![0; 3],
        };
        assert!(image.png_bytes().is_err());
    }
}
