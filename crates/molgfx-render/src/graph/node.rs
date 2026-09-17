//! Graph node and resource declarations.

use crate::graph::context::PassContext;
use molgfx_gpu::{Device, TextureFormat, TextureUsage};
use smallvec::SmallVec;

/// Identifies a declared transient resource within one graph.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ResourceId(pub u32);

impl ResourceId {
    /// The presentation target. Not a transient: it is acquired from the
    /// surface (or supplied as an off-screen target), never pooled.
    pub const SWAPCHAIN: Self = Self(u32::MAX);
}

/// How a transient texture is sized relative to the frame. Reduced-resolution
/// and fixed-size classes arrive with the passes that need them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SizeClass {
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
pub struct ResourceDesc {
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
pub enum PassKind {
    /// A render pass.
    Graphics,
    /// Compute dispatch.
    Compute,
}

/// One declared pass: name, dependencies, and its record function. The
/// record function is a plain function pointer — pass state (pipelines,
/// bind groups) lives in the registry the context exposes, so recording
/// stays free of captures and dynamic dispatch.
pub struct PassNode<D: Device> {
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
