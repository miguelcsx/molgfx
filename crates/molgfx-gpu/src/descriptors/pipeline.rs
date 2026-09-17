//! Pipeline descriptors.

use super::texture::TextureFormat;
use crate::device::Device;

/// Depth comparison functions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CompareFunction {
    /// Pass when the fragment depth is greater or equal — the default under
    /// reversed depth, where nearer means larger.
    GreaterEqual,
    /// Pass when strictly greater.
    Greater,
    /// Pass when less or equal (forward depth).
    LessEqual,
    /// Always pass.
    Always,
}

/// How a color target blends its output.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BlendMode {
    /// Overwrite the destination.
    #[default]
    Replace,
    /// Classic source-over alpha blending.
    Alpha,
    /// Additive accumulation.
    Additive,
    /// Multiply destination by (1 − source alpha); the revealage half of
    /// weighted-blended transparency.
    ReverseMultiply,
}

/// One color attachment a render pipeline writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ColorTarget {
    /// Attachment format.
    pub format: TextureFormat,
    /// Blend behavior.
    pub blend: BlendMode,
}

/// Depth behavior of a render pipeline.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DepthState {
    /// Depth attachment format.
    pub format: TextureFormat,
    /// Whether the pipeline writes depth.
    pub write: bool,
    /// The comparison against stored depth.
    pub compare: CompareFunction,
}

/// Primitive assembly.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PrimitiveTopology {
    /// Independent triangles — the only topology the impostor and mesh
    /// paths need.
    #[default]
    TriangleList,
}

/// Everything needed to create a render pipeline. Pipelines are created at
/// load and cached; never inside the frame loop.
#[derive(Debug)]
pub struct RenderPipelineDesc<'a, D: Device> {
    /// Debug label.
    pub label: &'static str,
    /// Bind-group layouts by group index (0 = per-frame, 1 = per-pass,
    /// 2 = per-representation, 3 = per-material); `None` leaves a
    /// frequency slot unused without renumbering the groups after it.
    pub layouts: &'a [Option<&'a D::BindGroupLayout>],
    /// The compiled module holding both entry points; modules are created
    /// once and shared across the pipelines that use them.
    pub shader: &'a D::ShaderModule,
    /// Vertex entry point name.
    pub vs_entry: &'static str,
    /// Fragment entry point name; `None` for depth-only pipelines.
    pub fs_entry: Option<&'static str>,
    /// Color targets, in attachment order.
    pub color_targets: &'a [ColorTarget],
    /// Depth behavior, if the pass has a depth attachment.
    pub depth: Option<DepthState>,
    /// Primitive assembly.
    pub topology: PrimitiveTopology,
    /// Values for the shader's `override` constants, by identifier.
    ///
    /// Specializing at pipeline creation is what lets a shader resolve a
    /// shape, a sample count or a feature switch once, instead of branching
    /// on it in every fragment. Empty leaves every override at its default.
    pub constants: &'a [(&'static str, f64)],
}

/// Everything needed to create a compute pipeline.
#[derive(Debug)]
pub struct ComputePipelineDesc<'a, D: Device> {
    /// Debug label.
    pub label: &'static str,
    /// Bind-group layouts by group index; `None` for unused slots.
    pub layouts: &'a [Option<&'a D::BindGroupLayout>],
    /// The compiled module.
    pub shader: &'a D::ShaderModule,
    /// Compute entry point name.
    pub entry: &'static str,
}
