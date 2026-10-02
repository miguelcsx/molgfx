//! The bounded scheduler that issues requests to a source.

use super::{Cancellation, Chunk, DataSource, Limits, Metadata, Request, SourceError};
use std::sync::atomic::{AtomicBool, Ordering};

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

pub(super) struct RequestGuard<'a>(&'a std::sync::atomic::AtomicUsize);

impl Drop for RequestGuard<'_> {
    fn drop(&mut self) {
        let _ = self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
