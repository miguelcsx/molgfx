//! Off-screen rendering and mapped publication images.
use super::{Engine, MotionBlur, QualityTier, TemporalOptions};
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
/// Publication samples that no tier refines any further.
pub(crate) const PUBLICATION_IMAGE_SAMPLES: u32 = 64;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImagePurpose {
    Publication,
    #[cfg(not(target_arch = "wasm32"))]
    SequenceFrame,
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
        let preparation = self.prepare_image(scene, purpose)?;
        let identity = scene.cache_identity();
        let scene_reset = self.temporal_scene_identity.replace(identity) != Some(identity);
        if purpose == ImagePurpose::Publication {
            self.temporal.reset();
        }
        let optics = self.resolve_optics(scene, camera)?;
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
        let samples = match purpose {
            // A sequence frame is one deterministic exposure per output frame.
            #[cfg(not(target_arch = "wasm32"))]
            ImagePurpose::SequenceFrame => 1,
            ImagePurpose::Publication => self.tier().image_samples(),
        };
        let shadow = self.shadow_bound.fit(
            scene,
            camera,
            self.resolved_plan.lighting(),
            preparation.scene_changed || preparation.rebuild,
        );
        let mut completion = FenceValue::default();
        for sample in 0..samples {
            self.scene_gpu.begin_frame();
            let cinematic = self.tier() >= QualityTier::Standard;
            let uniforms = self.temporal.prepare(
                camera,
                &TemporalOptions {
                    extent: [self.width, self.height],
                    reset: (purpose == ImagePurpose::Publication
                        || scene_reset
                        || preparation.rebuild)
                        && sample == 0,
                    quality: cinematic,
                    publication: purpose == ImagePurpose::Publication,
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
            self.scene_gpu
                .write_frame_uniforms(&self.queue, &uniforms)?;
            let mut encoder = self.device.create_command_encoder();
            self.record_image_scene_updates(&mut encoder, cinematic);
            self.record_image(&mut encoder, &view, None, cinematic, false);
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
            completion = self.submit_image_sample(encoder, sample + 1 == samples);
        }
        if purpose == ImagePurpose::Publication {
            self.temporal_scene_identity = None;
        }
        Ok(PendingImage {
            config,
            layout,
            buffer: readback,
            _texture: texture,
            completion,
        })
    }

    fn submit_image_sample(&self, encoder: D::CommandEncoder, final_sample: bool) -> FenceValue {
        if final_sample {
            self.queue.submit_tracked(encoder)
        } else {
            self.queue.submit(encoder);
            FenceValue::default()
        }
    }

    fn record_image_scene_updates(&mut self, encoder: &mut D::CommandEncoder, quality: bool) {
        self.passes
            .cull
            .record_attribute_timelines(&self.scene_gpu, encoder);
        self.passes
            .cull
            .record_instance_timelines(&self.scene_gpu, encoder);
        let point_coordinates_changed = self
            .passes
            .cull
            .record_point_timelines(&self.scene_gpu, encoder);
        self.scene_gpu
            .record_particle_motion(encoder, &self.passes.particle_motion);
        let structure_coordinates_changed =
            self.scene_gpu
                .record_trajectories(encoder, &self.passes.trajectory, None);
        let paged_coordinates_changed = self
            .passes
            .cull
            .record_paged_trajectories(&self.scene_gpu, encoder);
        self.scene_gpu.record_dynamic_relations(
            encoder,
            &self.passes.relation_resolve,
            structure_coordinates_changed || paged_coordinates_changed || point_coordinates_changed,
        );
        self.scene_gpu
            .record_occupancies(encoder, self.passes.occupancy.as_ref());
        self.scene_gpu.record_surface_fields(
            encoder,
            &self.passes.surface_field,
            &self.passes.surface_components,
        );
        self.scene_gpu.record_quality_hardware(encoder, quality);
    }

    /// Prepares the off-screen frame: scene sync, tier publication and pool
    /// rebuild.
    ///
    /// The adaptive loop is pinned only for publication. A sequence frame is a
    /// deterministic exposure of a caller-driven timeline, but the engine that
    /// renders it is still an interactive one, so its tiers keep adapting.
    pub(super) fn prepare_image(
        &mut self,
        scene: &Scene,
        purpose: ImagePurpose,
    ) -> Result<ImagePreparation, RenderError> {
        self.ensure_occupancy(scene)?;
        self.device.check_errors()?;
        if purpose == ImagePurpose::Publication {
            self.adaptive.set_publication(true);
        }
        self.sync_quality_tier();
        self.chunk_residency.begin_epoch();
        let scene_changed = self.scene_gpu.sync(crate::scene_gpu::SceneSync {
            device: &self.device,
            queue: &self.queue,
            scene,
            quality: self.tier() >= QualityTier::Standard,
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
    ) {
        self.record_image_until(encoder, target, queries, quality, timestamps_started, None);
    }

    pub(super) fn record_image_until(
        &self,
        encoder: &mut D::CommandEncoder,
        target: &D::TextureView,
        queries: Option<&D::QuerySet>,
        quality: bool,
        timestamps_started: bool,
        stop_after: Option<crate::graph::ResourceId>,
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
                    .map(|queries| molgfx_gpu::TimestampWrites {
                        queries,
                        beginning: (position == 0 && !timestamps_started).then_some(0),
                        end: (position + 1 == self.order.len()).then_some(1),
                    }),
                temporal_write: self.temporal.write_index(),
                quality,
                display_encoding: self.display_encoding(),
            });
            if stop_after.is_some_and(|resource| node.writes.contains(&resource)) {
                break;
            }
        }
    }
}

pub(super) struct PendingImage<D: Device> {
    config: ImageConfig,
    layout: ImageLayout,
    buffer: D::Buffer,
    _texture: D::Texture,
    completion: FenceValue,
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
        self,
        mapped: Vec<u8>,
        format: TextureFormat,
    ) -> Result<Image, RenderError> {
        let _completion = self.completion;
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
        })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "image_tests.rs"]
mod tests;
