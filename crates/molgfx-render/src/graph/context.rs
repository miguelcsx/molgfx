//! What a pass sees while recording.
//!
//! A pass gets this context, never the raw device: it can only touch the
//! resources the graph resolved for it, the per-frame state, and the pass
//! registry holding pipelines created at load.

use crate::engine::{DisplayGamut, TransferFunction};
use crate::error::RenderError;
use crate::graph::node::ResourceId;
use crate::graph::pool::TransientPool;
use crate::passes::{FrameBindings, Lazy, PassRegistry};
use crate::scene_gpu::GpuScene;
use molgfx_gpu::{Device, TextureFormat, TimestampWrites};

/// Resolves declared resource ids to concrete views for one frame.
pub(crate) struct ResourceTable<'a, D: Device> {
    pub(crate) pool: &'a TransientPool<D>,
    /// The acquired presentation target for this frame.
    pub(crate) swapchain: &'a D::TextureView,
}

impl<D: Device> ResourceTable<'_, D> {
    /// The view for a declared resource, or the frame's presentation target
    /// for [`ResourceId::SWAPCHAIN`].
    pub(crate) fn view(&self, id: ResourceId) -> Option<&D::TextureView> {
        if id == ResourceId::SWAPCHAIN {
            return Some(self.swapchain);
        }
        self.pool.view(id)
    }

    /// The physical texture behind a declared resource.
    pub(crate) fn texture(&self, id: ResourceId) -> Option<&D::Texture> {
        self.pool.texture(id)
    }

    /// The full-resolution frame extent.
    pub(crate) fn extent(&self) -> (u32, u32) {
        self.pool.extent()
    }
}

/// The display output format one frame is encoded for.
///
/// Both fields index pre-built tonemap pipelines, so the set of encodings is
/// closed and every variant exists before the first frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct DisplayEncoding {
    /// Output primaries.
    pub gamut: DisplayGamut,
    /// Output electro-optical transfer curve.
    pub transfer: TransferFunction,
}

/// What a pass needs to build itself the first time a frame draws with it.
pub(crate) struct PassEnv<'a, D: Device> {
    pub(crate) device: &'a D,
    pub(crate) target_format: TextureFormat,
    pub(crate) scene: &'a GpuScene<D>,
}

/// Everything a record function receives.
pub(crate) struct PassContext<'a, D: Device> {
    /// The open command encoder.
    pub encoder: &'a mut D::CommandEncoder,
    /// The device passes build themselves on.
    pub device: &'a D,
    /// The presentation format pipelines are specialized for.
    pub target_format: TextureFormat,
    /// The first failure a pass met while building; the frame reports it.
    pub failure: &'a mut Option<RenderError>,
    /// Resolves resource ids to views.
    pub resources: &'a ResourceTable<'a, D>,
    /// Pass state created at load: pipelines, layouts, bind groups.
    pub passes: &'a PassRegistry<D>,
    /// Bind groups rebuilt only when size-dependent transient views change.
    pub bindings: Option<&'a FrameBindings<D>>,
    /// GPU-resident scene state: buffers and bind groups.
    pub scene: &'a GpuScene<D>,
    /// Optional writes for the boundaries of this exact pass.
    pub timestamps: Option<TimestampWrites<'a, D>>,
    /// Ping-pong history texture written by this frame.
    pub temporal_write: usize,
    /// Whether progressive quality effects replace their realtime variants.
    pub quality: bool,
    /// Whether the tonemap pass smooths edges.
    ///
    /// Resolved from the presentation profile when it states a choice, and
    /// otherwise the realtime tiers smooth and the converged quality tiers do
    /// not, because accumulation already averages sub-pixel coverage.
    pub edge_smoothing: bool,
    /// Display gamut and transfer curve for this frame's presentation.
    ///
    /// The tonemap shader takes these as pipeline constants rather than
    /// decoding them per pixel, so the pass selects a pre-built variant here
    /// instead of branching in the fragment stage.
    pub display_encoding: DisplayEncoding,
}

impl<'a, D: Device> PassContext<'a, D> {
    /// A pass built on first use, or `None` once building it failed.
    ///
    /// A failure is kept for the frame to report, so a record function can
    /// simply draw nothing; the pass stays unbuilt and a later frame retries.
    pub(crate) fn build<'p, T>(
        &mut self,
        cell: &'p Lazy<T>,
        make: impl FnOnce(&PassEnv<'_, D>) -> Result<T, RenderError>,
    ) -> Option<&'p T> {
        let env = self.env();
        cell.build_in(&env, &mut *self.failure, make)
    }

    /// The device, format and scene a pass builds itself from.
    pub(crate) fn env(&self) -> PassEnv<'a, D> {
        PassEnv {
            device: self.device,
            target_format: self.target_format,
            scene: self.scene,
        }
    }
}
