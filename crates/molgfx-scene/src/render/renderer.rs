//! The backend-selected renderer and its frame, pick and profile entry points.

#[cfg(not(target_arch = "wasm32"))]
use super::FrameTiming;
#[cfg(not(target_arch = "wasm32"))]
use super::Image;
use super::engine_config::engine_config;
use super::{FrameReport, PassTiming};
use crate::{Error, RenderProfile, Scene};
#[cfg(not(target_arch = "wasm32"))]
use num_traits::ToPrimitive as _;

/// Backend-selected renderer owning device resources and physical caches.
#[derive(Debug)]
pub struct Renderer {
    pub(super) inner: molgfx_render::Engine<molgfx_wgpu::WgpuDevice>,
    pub(super) profile: RenderProfile,
}

impl Renderer {
    /// Opens the preferred device with the adaptive interactive profile.
    ///
    /// # Errors
    ///
    /// Returns a typed renderer error when no compatible device can be opened.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new() -> Result<Self, Error> {
        Self::with_profile(RenderProfile::default())
    }

    /// Opens the preferred device with an explicit high-level profile.
    ///
    /// # Errors
    ///
    /// Returns a typed renderer error when no compatible device can be opened.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_profile(profile: RenderProfile) -> Result<Self, Error> {
        Self::with_surface_budget(profile, None)
    }

    /// Opens the preferred device with an explicit profile and a device-memory
    /// budget for one implicit-surface field, in bytes.
    ///
    /// A surface that would not fit is sampled more coarsely; the output reports
    /// the spacing it used. `None` keeps the default of 384 MiB, which holds a
    /// protein of about 80 Å at the finest spacing.
    ///
    /// # Errors
    ///
    /// Returns a typed renderer error when no compatible device can be opened.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_surface_budget(
        profile: RenderProfile,
        surface_field_budget_bytes: Option<u64>,
    ) -> Result<Self, Error> {
        let config = engine_config(profile, surface_field_budget_bytes);
        let inner = molgfx_render::Engine::new(&config, None)?;
        Ok(Self { inner, profile })
    }

    /// Opens WebGPU asynchronously against a browser-owned canvas.
    ///
    /// # Errors
    ///
    /// Returns a typed renderer error when the adapter or pipelines fail.
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    pub async fn for_canvas(
        canvas: web_sys::HtmlCanvasElement,
        profile: RenderProfile,
    ) -> Result<Self, Error> {
        let config = engine_config(profile, None);
        let inner = molgfx_render::Engine::new_async(&config, Some(canvas)).await?;
        Ok(Self { inner, profile })
    }

    /// Resizes the current presentation target.
    pub fn resize(&mut self, size: (u32, u32)) {
        self.inner.resize(size.0.max(1), size.1.max(1));
    }

    /// Renders one frame to the attached presentation target.
    ///
    /// # Errors
    ///
    /// Returns a typed surface, device, or scene synchronization error.
    pub fn present(
        &mut self,
        scene: &Scene,
        camera: &molgfx_math::Camera,
    ) -> Result<FrameReport, Error> {
        self.inner
            .render(scene.resolved(), camera)
            .map_err(Error::from)
    }

    /// Renders one deterministic off-screen image, inferring a framing camera.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty target or a renderer/device failure.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_image(&mut self, scene: &Scene, size: (u32, u32)) -> Result<Image, Error> {
        if size.0 == 0 || size.1 == 0 {
            return Err(Error::InvalidSpec(
                "image width and height must be non-zero".to_owned(),
            ));
        }
        let width = size
            .0
            .to_f32()
            .ok_or_else(|| Error::InvalidSpec("image width cannot be represented".to_owned()))?;
        let height = size
            .1
            .to_f32()
            .ok_or_else(|| Error::InvalidSpec("image height cannot be represented".to_owned()))?;
        let aspect = width / height;
        let camera = scene.framing_camera(aspect);
        self.render_image_with_camera(scene, &camera, size)
    }

    /// Renders one deterministic off-screen image with an explicit camera.
    ///
    /// # Errors
    ///
    /// Returns a typed renderer or device error.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_image_with_camera(
        &mut self,
        scene: &Scene,
        camera: &molgfx_math::Camera,
        size: (u32, u32),
    ) -> Result<Image, Error> {
        self.inner
            .render_image(
                scene.resolved(),
                camera,
                molgfx_render::ImageConfig {
                    width: size.0,
                    height: size.1,
                },
            )
            .map(Image)
            .map_err(Error::from)
    }

    /// Measures one fully converged output with a framing camera.
    ///
    /// All exposure samples are included; discard warmup outputs before
    /// percentile gates. Pixel export is outside this measurement.
    ///
    /// # Errors
    ///
    /// Returns a typed renderer or device error. Missing timestamp capability
    /// leaves GPU timing unresolved while the completion fence is still awaited.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn measure_frame(&mut self, scene: &Scene, size: (u32, u32)) -> Result<FrameTiming, Error> {
        let (Some(width), Some(height)) = (size.0.to_f32(), size.1.to_f32()) else {
            return Err(Error::InvalidSpec(
                "image size cannot be represented".to_owned(),
            ));
        };
        let aspect = width / height.max(1.0);
        let camera = scene.framing_camera(aspect);
        self.measure_frame_with_camera(scene, &camera, size)
    }

    /// Measures a converged output using the caller's physical camera.
    ///
    /// # Errors
    ///
    /// Returns invalid extent, rendering or device errors.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn measure_frame_with_camera(
        &mut self,
        scene: &Scene,
        camera: &molgfx_math::Camera,
        size: (u32, u32),
    ) -> Result<FrameTiming, Error> {
        self.inner
            .profile_frame(
                scene.resolved(),
                camera,
                molgfx_render::ImageConfig {
                    width: size.0,
                    height: size.1,
                },
            )
            .map_err(Error::from)
    }

    /// Renders a bounded sequence of deterministic frames, one per timestamp.
    ///
    /// Each frame is a fully converged publication image, so a sequence of
    /// `N` frames costs `N` publication renders. Frames are submitted without
    /// waiting and resolved in order, so the caller can drive a camera path and
    /// read the completed frames back at their own pace.
    ///
    /// Movie *encoding* is deliberately not here: the engine produces the
    /// frames, and a caller that wants an MP4 or a GIF encodes them with
    /// whatever tool it already uses.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for an empty target, a zero or
    /// non-representable frame rate, or a non-increasing timestamp, and a
    /// typed renderer or device error otherwise.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_sequence(
        &mut self,
        scene: &Scene,
        size: (u32, u32),
        frames_per_second: u32,
        frames: usize,
    ) -> Result<Vec<Image>, Error> {
        if size.0 == 0 || size.1 == 0 {
            return Err(Error::InvalidSpec(
                "frame width and height must be non-zero".to_owned(),
            ));
        }
        let (Some(width), Some(height)) = (size.0.to_f32(), size.1.to_f32()) else {
            return Err(Error::InvalidSpec(
                "frame size cannot be represented".to_owned(),
            ));
        };
        let camera = scene.framing_camera(width / height.max(1.0));
        self.render_frames(scene, size, frames_per_second, frames, |_| Ok(camera))
    }

    /// Renders one converged frame per tick of a camera path, in order.
    ///
    /// Frame `i` shows the path at `start + i / frames_per_second` seconds, so
    /// the sequence covers the path's own time range inclusively. Each frame is a
    /// complete publication render; encoding the frames into a movie is the
    /// caller's job.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for an empty target, a zero frame
    /// rate, or a range that yields no frame, and a typed renderer error otherwise.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_camera_path(
        &mut self,
        scene: &Scene,
        path: &crate::camera::CameraPath,
        size: (u32, u32),
        frames_per_second: u32,
    ) -> Result<Vec<Image>, Error> {
        if size.0 == 0 || size.1 == 0 || frames_per_second == 0 {
            return Err(Error::InvalidSpec(
                "frame size and frame rate must be non-zero".to_owned(),
            ));
        }
        let [start, end] = path.range();
        let count = ((end - start) * f64::from(frames_per_second)).floor();
        let frames = count
            .to_usize()
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| {
                Error::InvalidSpec("the path yields no representable frame count".to_owned())
            })?;
        let step = 1.0 / f64::from(frames_per_second);
        self.render_frames(scene, size, frames_per_second, frames, |index| {
            let offset = index
                .to_f64()
                .ok_or_else(|| Error::InvalidSpec("frame index is not representable".to_owned()))?;
            path.sample(start + step * offset)
                .ok_or_else(|| Error::InvalidSpec("the path cannot be sampled".to_owned()))
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn render_frames(
        &mut self,
        scene: &Scene,
        size: (u32, u32),
        frames_per_second: u32,
        frames: usize,
        camera_at: impl Fn(usize) -> Result<crate::Camera, Error>,
    ) -> Result<Vec<Image>, Error> {
        let config = molgfx_render::SequenceConfig::at_fps(
            molgfx_render::ImageConfig {
                width: size.0,
                height: size.1,
            },
            frames_per_second,
            2,
        )
        .map_err(Error::from)?;
        let mut sequence = self.inner.sequence(config).map_err(Error::from)?;
        let in_flight = usize::from(molgfx_render::Engine::sequence_in_flight(&sequence));
        let mut images = Vec::with_capacity(frames);
        for index in 0..frames {
            let timestamp = u64::try_from(index)
                .ok()
                .and_then(|index| index.checked_mul(config.timebase_nanoseconds))
                .ok_or_else(|| {
                    Error::InvalidSpec("sequence timestamp exceeds the supported range".to_owned())
                })?;
            // Drain before the pipeline is full, so a long sequence streams
            // instead of stalling at the in-flight limit.
            while molgfx_render::Engine::pending_sequence_frames(&sequence) >= in_flight {
                let frame = self
                    .inner
                    .drain_sequence_frame(&mut sequence)
                    .map_err(Error::from)?;
                images.push(Image(frame.image));
            }
            self.inner
                .submit_sequence_frame(
                    &mut sequence,
                    scene.resolved(),
                    &camera_at(index)?,
                    timestamp,
                )
                .map_err(Error::from)?;
        }
        let resolved = self.inner.finish_sequence(sequence).map_err(Error::from)?;
        images.extend(resolved.into_iter().map(|frame| Image(frame.image)));
        Ok(images)
    }

    /// Deterministic description of the high-level policy and semantic scene.
    #[must_use]
    pub fn explain(&self, scene: &Scene) -> String {
        format!(
            "{}\ntarget fps: {}\nquality: {:?}\n{}",
            scene.explain(),
            self.profile.target_fps,
            self.profile.quality,
            self.inner.explain()
        )
    }
}

impl Renderer {
    /// Actual pass timings from the latest completed profile, without a copy.
    #[must_use]
    pub fn last_pass_timings(&self) -> &[PassTiming] {
        self.inner.last_pass_timings()
    }
}
