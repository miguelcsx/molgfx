//! What a pass sees while recording.
//!
//! A pass gets this context, never the raw device: it can only touch the
//! resources the graph resolved for it, the per-frame state, and the pass
//! registry holding pipelines created at load.

use crate::engine::{DisplayGamut, TransferFunction};
use crate::graph::node::ResourceId;
use crate::graph::pool::TransientPool;
use crate::passes::{FrameBindings, PassRegistry};
use crate::scene_gpu::GpuScene;
use pdviewx_gpu::{Device, TimestampWrites};

/// Resolves declared resource ids to concrete views for one frame.
pub struct ResourceTable<'a, D: Device> {
    pub(crate) pool: &'a TransientPool<D>,
    /// The acquired presentation target for this frame.
    pub(crate) swapchain: &'a D::TextureView,
}

impl<D: Device> ResourceTable<'_, D> {
    /// The view for a declared resource, or the frame's presentation target
    /// for [`ResourceId::SWAPCHAIN`].
    pub fn view(&self, id: ResourceId) -> Option<&D::TextureView> {
        if id == ResourceId::SWAPCHAIN {
            return Some(self.swapchain);
        }
        self.pool.view(id)
    }
}

/// The display output format one frame is encoded for.
///
/// Both fields index pre-built tonemap pipelines, so the set of encodings is
/// closed and every variant exists before the first frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DisplayEncoding {
    /// Output primaries.
    pub gamut: DisplayGamut,
    /// Output electro-optical transfer curve.
    pub transfer: TransferFunction,
}

/// Everything a record function receives.
pub struct PassContext<'a, D: Device> {
    /// The open command encoder.
    pub encoder: &'a mut D::CommandEncoder,
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
    /// Whether the resolved graph contains the physical camera pass.
    pub depth_of_field: bool,
    /// Whether the resolved graph contains the camera-shutter gather.
    pub motion_blur: bool,
    /// Display gamut and transfer curve for this frame's presentation.
    ///
    /// The tonemap shader takes these as pipeline constants rather than
    /// decoding them per pixel, so the pass selects a pre-built variant here
    /// instead of branching in the fragment stage.
    pub display_encoding: DisplayEncoding,
}
