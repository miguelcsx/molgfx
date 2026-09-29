//! Physical renderer wrapper with target-sized rendering.

use crate::{Error, Quality, RenderProfile, Scene};
pub use molgfx_render::{FrameReport, FrameTiming};
#[cfg(not(target_arch = "wasm32"))]
use num_traits::ToPrimitive as _;

/// Semantic entity namespace returned by picking.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PickKind {
    /// Molecular atom.
    Atom,
    /// Molecular bond.
    Bond,
    /// Detected or caller-supplied interaction.
    Interaction,
    /// Annotation label.
    Label,
    /// Distance, angle or dihedral measurement.
    Measurement,
    /// Analytic extension primitive.
    Primitive,
    /// Mesh extension entity.
    Mesh,
    /// Ligand-pose candidate batch.
    LigandPoseBatch,
    /// Analytic guide.
    Guide,
    /// Time-dependent bond.
    DynamicBond,
    /// Data-extension point.
    Point,
    /// Shared-template occurrence.
    Instance,
    /// Part of a shared-template occurrence.
    TemplatePart,
    /// Data-extension relation.
    Relation,
    /// Categorical volume segment.
    VolumeSegment,
}

/// Stable semantic provenance resolved from one physical GPU token.
#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct PickResult {
    /// Entity namespace.
    pub kind: PickKind,
    /// Provider dataset identity for molecular and extension entities.
    pub dataset: Option<u64>,
    /// Provider chunk identity when data is streamed.
    pub chunk: Option<u64>,
    /// Stable logical source row.
    pub row: Option<u64>,
    /// Exact categorical label for a volume segment.
    pub volume_label: Option<u32>,
}

impl PickResult {
    /// Deterministic JSON payload used by browser events.
    ///
    /// # Errors
    ///
    /// Returns an error only if serialization of the fixed schema fails.
    pub fn to_json(&self) -> Result<String, Error> {
        serde_json::to_string(self).map_err(Error::from)
    }
}

/// Encoded image returned by off-screen rendering.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct Image(molgfx_render::Image);

#[cfg(not(target_arch = "wasm32"))]
impl Image {
    /// Row-major RGBA8 pixels with no row padding.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.0.pixels
    }

    /// Encodes the image as PNG.
    ///
    /// # Errors
    ///
    /// Returns an image encoding error if the pixels cannot be encoded.
    pub fn png_bytes(&self) -> Result<Vec<u8>, Error> {
        self.0.png_bytes().map_err(Error::from)
    }

    /// Writes a PNG image to disk.
    ///
    /// # Errors
    ///
    /// Returns an encoding or filesystem error.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<(), Error> {
        let file = std::fs::File::create(path)?;
        self.0
            .write_png(std::io::BufWriter::new(file))
            .map_err(Error::from)
    }

    /// Pixel width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.0.width
    }

    /// Pixel height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.0.height
    }
}

/// Backend-selected renderer owning device resources and physical caches.
#[derive(Debug)]
pub struct Renderer {
    inner: molgfx_render::Engine<molgfx_wgpu::WgpuDevice>,
    profile: RenderProfile,
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
        let config = engine_config(profile);
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
        let config = engine_config(profile);
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

    /// Asynchronously resolves the entity under one target pixel.
    ///
    /// # Errors
    ///
    /// Returns a typed readback or device error.
    pub async fn pick_async(&mut self, x: u32, y: u32) -> Result<Option<PickResult>, Error> {
        self.inner
            .pick_async(x, y)
            .await
            .map(|pick| pick.as_ref().map(semantic_pick))
            .map_err(Error::from)
    }

    /// Resolves the entity under one target pixel on native platforms.
    ///
    /// # Errors
    ///
    /// Returns a typed readback or device error.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn pick(&mut self, x: u32, y: u32) -> Result<Option<PickResult>, Error> {
        self.inner
            .pick(x, y)
            .map(|pick| pick.as_ref().map(semantic_pick))
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

    /// Measures one frame with device timestamp queries.
    ///
    /// The frame is rendered at the renderer's current quality tier, so this is
    /// the cost of one interactive frame at that tier, not of a converged
    /// publication image. Discard warmup frames before averaging.
    ///
    /// # Errors
    ///
    /// Returns a capability error when the adapter exposes no timestamps, or a
    /// typed renderer or device error.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn measure_frame(&mut self, scene: &Scene, size: (u32, u32)) -> Result<FrameTiming, Error> {
        let (Some(width), Some(height)) = (size.0.to_f32(), size.1.to_f32()) else {
            return Err(Error::InvalidSpec(
                "image size cannot be represented".to_owned(),
            ));
        };
        let aspect = width / height.max(1.0);
        let camera = scene.framing_camera(aspect);
        self.inner
            .profile_frame(
                scene.resolved(),
                &camera,
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
        let aspect = width / height.max(1.0);
        let camera = scene.framing_camera(aspect);
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
                .submit_sequence_frame(&mut sequence, scene.resolved(), &camera, timestamp)
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

fn semantic_pick(pick: &molgfx_render::Pick) -> PickResult {
    match pick.entity {
        molgfx_render::PickEntity::Structure(identity) => PickResult {
            kind: pick_kind(identity.kind()),
            dataset: Some(identity.dataset().get()),
            chunk: Some(identity.chunk().get()),
            row: Some(identity.row().get()),
            volume_label: None,
        },
        molgfx_render::PickEntity::VolumeSegment(segment) => PickResult {
            kind: PickKind::VolumeSegment,
            dataset: None,
            chunk: None,
            row: None,
            volume_label: Some(segment.label),
        },
    }
}

const fn pick_kind(kind: molgfx_core::EntityKind) -> PickKind {
    match kind {
        molgfx_core::EntityKind::Atom => PickKind::Atom,
        molgfx_core::EntityKind::Bond => PickKind::Bond,
        molgfx_core::EntityKind::Edge => PickKind::Interaction,
        molgfx_core::EntityKind::Label => PickKind::Label,
        molgfx_core::EntityKind::Measurement => PickKind::Measurement,
        molgfx_core::EntityKind::Primitive => PickKind::Primitive,
        molgfx_core::EntityKind::Mesh => PickKind::Mesh,
        molgfx_core::EntityKind::LigandPoseBatch => PickKind::LigandPoseBatch,
        molgfx_core::EntityKind::Guide => PickKind::Guide,
        molgfx_core::EntityKind::DynamicBond => PickKind::DynamicBond,
        molgfx_core::EntityKind::Point => PickKind::Point,
        molgfx_core::EntityKind::Instance => PickKind::Instance,
        molgfx_core::EntityKind::TemplatePart => PickKind::TemplatePart,
        molgfx_core::EntityKind::Relation => PickKind::Relation,
    }
}

fn engine_config(profile: RenderProfile) -> molgfx_render::EngineConfig {
    let mut presentation = match profile.quality {
        Quality::Publication => molgfx_render::RenderProfile::illustrative(),
        Quality::Auto | Quality::Interactive => molgfx_render::RenderProfile::inspection(),
    };
    if let Some(cue) = profile.depth_cue {
        presentation = presentation.with_effect(molgfx_render::PresentationEffect::DepthCue(
            molgfx_render::DepthCue {
                near_distance: cue.near_distance(),
                far_distance: cue.far_distance(),
                strength: cue.strength(),
            },
        ));
    }
    if let Some(edge_smoothing) = profile.edge_smoothing {
        presentation = presentation.with_effect(molgfx_render::PresentationEffect::AntiAliasing(
            molgfx_render::AntiAliasingStyle { edge_smoothing },
        ));
    }
    molgfx_render::EngineConfig {
        profile: presentation,
        // Only the adaptive policy adapts. `Interactive` is an explicit request
        // for low latency that still renders at one fixed tier, and publication
        // output must stay reproducible, so neither may hold a tier that
        // depends on how fast the machine happens to be.
        adaptive: molgfx_render::AdaptiveQualityConfig {
            target_fps: profile.target_fps,
            enabled: profile.quality == Quality::Auto,
        },
        ..molgfx_render::EngineConfig::default()
    }
}

#[cfg(test)]
mod tests;
