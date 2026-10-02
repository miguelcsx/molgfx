//! The provider contract the scheduler drives.

use super::{Cancellation, Chunk, Metadata, Request, SourceError};

/// High-level asynchronous source; scheduling, caching and upload stay in the renderer.
pub trait DataSource: Send + Sync {
    /// Reads immutable dataset metadata.
    fn metadata(&self) -> impl Future<Output = Result<Metadata, SourceError>> + Send;

    /// Fetches one bounded batch with cooperative cancellation.
    fn request(
        &self,
        request: Request,
        cancellation: Cancellation,
    ) -> impl Future<Output = Result<Vec<Chunk>, SourceError>> + Send;

    /// Releases provider resources and rejects subsequent work.
    fn shutdown(&self) -> impl Future<Output = Result<(), SourceError>> + Send;
}
