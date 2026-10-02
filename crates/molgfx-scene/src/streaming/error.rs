//! Why a source could not deliver a chunk.

/// Streaming failure propagated to the renderer.
#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
pub enum SourceError {
    /// The request was cancelled before publication.
    #[error("stream request was cancelled")]
    Cancelled,
    /// The provider is shutting down.
    #[error("stream source is shut down")]
    Shutdown,
    /// Provider-specific failure.
    #[error("stream source failed: {0}")]
    Provider(Box<str>),
    /// A provider violated the bounded batch contract.
    #[error("stream source returned an invalid batch: {0}")]
    InvalidBatch(Box<str>),
    /// The renderer-owned in-flight bound is currently saturated.
    #[error("stream source is applying backpressure")]
    Backpressure,
}
