//! The render error type.

use thiserror::Error;

/// Everything that can go wrong constructing or driving the engine.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RenderError {
    /// A device-level failure, carried upward with its own code.
    #[error(transparent)]
    Gpu(#[from] pdviewx_gpu::GpuError),

    /// The render graph contains a dependency cycle.
    #[error("render graph cycle involving pass {pass:?}")]
    GraphCycle {
        /// A pass on the cycle.
        pass: &'static str,
    },

    /// A pass referenced a resource id the graph never declared.
    #[error("pass {pass:?} references undeclared resource {resource}")]
    UnknownResource {
        /// The offending pass.
        pass: &'static str,
        /// The undeclared id.
        resource: u32,
    },

    /// Off-screen dimensions overflowed a portable texture/readback layout.
    #[error("invalid off-screen image dimensions")]
    InvalidImageSize,

    /// A scene-tracked optical target is stale, empty, or behind the camera.
    #[error("invalid optical focus target: {reason}")]
    InvalidFocusTarget {
        /// Stable, caller-actionable reason.
        reason: &'static str,
    },

    /// The mapped image could not be encoded for publication.
    #[error("image encoding failed: {summary}")]
    ImageEncoding {
        /// Encoder-provided diagnostic.
        summary: String,
    },

    /// A render session could not be encoded or decoded.
    #[error("render session encoding failed: {summary}")]
    SessionEncoding {
        /// Serializer-provided diagnostic.
        summary: String,
    },
}

impl RenderError {
    /// The stable registry code for this condition.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Gpu(e) => e.code(),
            Self::GraphCycle { .. } => "PDVIEWX-E0070",
            Self::UnknownResource { .. } => "PDVIEWX-E0071",
            Self::InvalidImageSize => "PDVIEWX-E0072",
            Self::InvalidFocusTarget { .. } => "PDVIEWX-E0073",
            Self::ImageEncoding { .. } => "PDVIEWX-E0074",
            Self::SessionEncoding { .. } => "PDVIEWX-E0075",
        }
    }
}
