//! Bounded asynchronous molecular data streaming.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Priority class used by the renderer's prefetch scheduler.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    /// Required for the next visible frame.
    Visible,
    /// Predicted to become visible soon.
    Prefetch,
    /// Opportunistic background request.
    Background,
}

/// Provider-neutral dataset metadata.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Metadata {
    /// Stable dataset identity.
    pub identity: Box<str>,
    /// Total logical chunks when known.
    pub chunk_count: Option<u64>,
    /// Approximate uncompressed bytes when known.
    pub byte_length: Option<u64>,
}

/// One prioritized batch of logical chunks.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Request {
    /// Logical chunk identities in provider order.
    pub chunks: Vec<u64>,
    /// Scheduling priority.
    pub priority: Priority,
}

/// One immutable binary chunk returned by a provider.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Chunk {
    /// Logical chunk identity.
    pub id: u64,
    /// Provider-owned bytes shared without an extra copy.
    pub bytes: Arc<[u8]>,
}

/// Cooperative cancellation shared across one request batch.
#[derive(Clone, Debug, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    /// Requests cancellation.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

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

/// Hard scheduler bounds independent of provider behavior.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    /// Maximum chunk identities in one provider call.
    pub maximum_batch: usize,
    /// Maximum concurrently active provider calls.
    pub maximum_in_flight: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            maximum_batch: 16,
            maximum_in_flight: 4,
        }
    }
}

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

/// Renderer-owned guard enforcing bounded request batches and shutdown.
#[derive(Debug)]
pub struct Scheduler<S> {
    source: S,
    limits: Limits,
    in_flight: std::sync::atomic::AtomicUsize,
    shutdown: AtomicBool,
}

impl<S: DataSource> Scheduler<S> {
    /// Wraps a source with a non-zero maximum chunks per request.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::InvalidBatch`] for a zero batch bound.
    pub fn new(source: S, limits: Limits) -> Result<Self, SourceError> {
        if limits.maximum_batch == 0 || limits.maximum_in_flight == 0 {
            return Err(SourceError::InvalidBatch(
                "scheduler bounds must be non-zero".into(),
            ));
        }
        Ok(Self {
            source,
            limits,
            in_flight: std::sync::atomic::AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
        })
    }

    /// Reads metadata unless shutdown has begun.
    ///
    /// # Errors
    ///
    /// Propagates shutdown and provider failures.
    pub async fn metadata(&self) -> Result<Metadata, SourceError> {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(SourceError::Shutdown);
        }
        self.source.metadata().await
    }

    /// Fetches one validated bounded batch.
    ///
    /// # Errors
    ///
    /// Propagates cancellation, shutdown, provider, and contract errors.
    pub async fn request(
        &self,
        request: Request,
        cancellation: Cancellation,
    ) -> Result<Vec<Chunk>, SourceError> {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(SourceError::Shutdown);
        }
        if request.chunks.is_empty() || request.chunks.len() > self.limits.maximum_batch {
            return Err(SourceError::InvalidBatch(
                "request exceeds configured bounds".into(),
            ));
        }
        if cancellation.is_cancelled() {
            return Err(SourceError::Cancelled);
        }
        let _guard = self.begin_request()?;
        let expected = request.chunks.clone();
        let chunks = self.source.request(request, cancellation.clone()).await?;
        if self.shutdown.load(Ordering::Acquire) {
            return Err(SourceError::Shutdown);
        }
        if cancellation.is_cancelled() {
            return Err(SourceError::Cancelled);
        }
        if chunks.len() != expected.len()
            || chunks
                .iter()
                .zip(expected)
                .any(|(chunk, id)| chunk.id != id)
        {
            return Err(SourceError::InvalidBatch(
                "provider changed request order or cardinality".into(),
            ));
        }
        Ok(chunks)
    }

    /// Shuts down exactly once.
    ///
    /// # Errors
    ///
    /// Propagates a provider shutdown failure.
    pub async fn shutdown(&self) -> Result<(), SourceError> {
        if self.shutdown.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        self.source.shutdown().await
    }

    fn begin_request(&self) -> Result<RequestGuard<'_>, SourceError> {
        let mut active = self.in_flight.load(Ordering::Acquire);
        loop {
            if active >= self.limits.maximum_in_flight {
                return Err(SourceError::Backpressure);
            }
            match self.in_flight.compare_exchange_weak(
                active,
                active + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(RequestGuard(&self.in_flight)),
                Err(current) => active = current,
            }
        }
    }
}

struct RequestGuard<'a>(&'a std::sync::atomic::AtomicUsize);

impl Drop for RequestGuard<'_> {
    fn drop(&mut self) {
        let _ = self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
