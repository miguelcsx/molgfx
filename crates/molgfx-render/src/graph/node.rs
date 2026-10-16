//! Graph node and resource declarations.

use crate::graph::context::PassContext;
use molgfx_gpu::{Device, TextureFormat, TextureUsage};
use smallvec::SmallVec;

/// Identifies a declared transient resource within one graph.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ResourceId(pub u32);

impl ResourceId {
    /// The presentation target. Not a transient: it is acquired from the
    /// surface (or supplied as an off-screen target), never pooled.
    pub(crate) const SWAPCHAIN: Self = Self(u32::MAX);
}

/// How a transient texture is sized relative to the frame. Reduced-resolution
/// and fixed-size classes arrive with the passes that need them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum SizeClass {
    /// The frame's full resolution.
    Full,
    /// One texel classifies each 16×16 full-resolution tile.
    Tiles16,
    /// Fixed-resolution scene-fit directional shadow map.
    Shadow,
    /// Half resolution on each axis twice over: a quarter-width, quarter-height
    /// target for wide blurs that never need full-resolution detail.
    Quarter,
}

/// A declared transient texture: described here, allocated from the pool.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct ResourceDesc {
    /// Debug label.
    pub label: &'static str,
    /// Texel format.
    pub format: TextureFormat,
    /// Sizing rule.
    pub size: SizeClass,
    /// Usages the passes need.
    pub usage: TextureUsage,
    /// Whether contents from the previous frame must survive until first use.
    pub persistent: bool,
}

/// Whether a pass records draws or dispatches. The compute kind arrives
/// with the cull pass.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PassKind {
    /// A render pass.
    Graphics,
    /// Compute dispatch.
    Compute,
}

/// Which samples of a multi-sample exposure record a pass.
///
/// Passes whose output is the same on every sample of one exposure run on the
/// first; passes that only turn accumulated history into the displayed image
/// run on the last.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SampleCadence {
    /// Every sample.
    Every,
    /// The first sample only: the result holds for the whole exposure.
    First,
    /// The last sample only: the result is the displayed image.
    Last,
}

/// Where a sample sits in its exposure.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct SamplePosition {
    pub first: bool,
    pub last: bool,
}

impl SamplePosition {
    /// A frame that is its own exposure.
    pub(crate) const ONLY: Self = Self {
        first: true,
        last: true,
    };

    /// Whether a pass of this cadence records at this position.
    pub(crate) const fn runs(self, cadence: SampleCadence) -> bool {
        match cadence {
            SampleCadence::Every => true,
            SampleCadence::First => self.first,
            SampleCadence::Last => self.last,
        }
    }
}

/// One declared pass: name, dependencies, and its record function. The
/// record function is a plain function pointer — pass state (pipelines,
/// bind groups) lives in the registry the context exposes, so recording
/// stays free of captures and dynamic dispatch.
pub(crate) struct PassNode<D: Device> {
    /// Stable name; doubles as the debug label.
    pub name: &'static str,
    /// Resources read.
    pub reads: SmallVec<[ResourceId; 4]>,
    /// Resources written.
    pub writes: SmallVec<[ResourceId; 4]>,
    /// Draw or dispatch.
    pub kind: PassKind,
    /// Records the pass's commands.
    pub record: fn(&mut PassContext<'_, D>),
    /// Which samples of a multi-sample exposure record the pass.
    pub cadence: SampleCadence,
}

impl<D: Device> std::fmt::Debug for PassNode<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassNode")
            .field("name", &self.name)
            .field("reads", &self.reads)
            .field("writes", &self.writes)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}
