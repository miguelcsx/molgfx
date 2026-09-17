//! Render and compute pass descriptors.

use crate::device::Device;

/// How a color attachment starts the pass.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LoadOp {
    /// Clear to the given linear RGBA value.
    Clear([f64; 4]),
    /// Keep the existing contents.
    Load,
}

/// How a depth attachment starts the pass.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DepthLoadOp {
    /// Clear to the given depth. Under reversed depth the empty scene
    /// clears to 0.0 (the far plane).
    Clear(f32),
    /// Keep the existing contents.
    Load,
}

/// One color attachment of a render pass.
#[derive(Debug)]
pub struct ColorAttachment<'a, D: Device> {
    /// The target view.
    pub view: &'a D::TextureView,
    /// Load behavior.
    pub load: LoadOp,
}

/// The depth attachment of a render pass.
#[derive(Debug)]
pub struct DepthAttachment<'a, D: Device> {
    /// The target view.
    pub view: &'a D::TextureView,
    /// Load behavior.
    pub load: DepthLoadOp,
    /// Whether the pass only tests/samples depth and never stores it.
    pub read_only: bool,
}

/// Everything needed to begin a render pass.
#[derive(Debug)]
pub struct RenderPassDesc<'a, D: Device> {
    /// Debug label.
    pub label: &'static str,
    /// Color attachments in order.
    pub colors: &'a [ColorAttachment<'a, D>],
    /// Optional depth attachment.
    pub depth: Option<DepthAttachment<'a, D>>,
    /// Optional timestamps written at the actual pass boundaries.
    pub timestamps: Option<TimestampWrites<'a, D>>,
}

/// Everything needed to begin a compute pass.
#[derive(Debug)]
pub struct ComputePassDesc<'a, D: Device> {
    /// Debug label.
    pub label: &'static str,
    /// Optional timestamps written at the actual pass boundaries.
    pub timestamps: Option<TimestampWrites<'a, D>>,
}

/// Timestamp query writes attached to one render or compute pass.
#[derive(Debug)]
pub struct TimestampWrites<'a, D: Device> {
    /// Destination query set.
    pub queries: &'a D::QuerySet,
    /// Query written as the pass begins.
    pub beginning: Option<u32>,
    /// Query written as the pass ends.
    pub end: Option<u32>,
}

impl<D: Device> Copy for TimestampWrites<'_, D> {}

impl<D: Device> Clone for TimestampWrites<'_, D> {
    fn clone(&self) -> Self {
        *self
    }
}
