//! Typed failures for bounded paged picking.

use super::picking::ResidentPage;
use crate::{ChunkSpan, LocalRow};
use thiserror::Error;

/// Typed failures from the bounded picking resolver.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Error)]
#[non_exhaustive]
pub enum PickingError {
    /// A resolver without slots cannot service picks.
    #[error("picking resolver capacity must be non-zero")]
    EmptyCapacity,
    /// The all-ones page value is reserved for an empty GPU token.
    #[error("picking resolver capacity exceeds the GPU page token space")]
    CapacityTooLarge,
    /// Fixed resolver storage could not be allocated.
    #[error("picking resolver storage allocation failed")]
    AllocationFailed,
    /// Every readback buffer is awaiting an earlier pick.
    #[error("every pick readback is already in flight")]
    InFlightExhausted,
    /// Every slot belongs to the current resident working set.
    #[error(
        "picking resolver working set is full (raise EngineConfig::picking_page_capacity or reduce resident chunks)"
    )]
    WorkingSetFull,
    /// The scene needs more resident pick pages than the engine can ever hold.
    #[error(
        "the scene needs more than {limit} resident picking pages (one per placed structure \
         and entity kind, per resident chunk, and per generic batch): place fewer structures or \
         merge them into instance batches"
    )]
    PageLimit {
        /// Hard ceiling on resident pick pages.
        limit: u32,
    },
    /// The same dataset, chunk and namespace already owns a page.
    #[error("picking page is already requested or resident")]
    DuplicatePage,
    /// The ticket or token names a page outside this resolver.
    #[error("resident picking page {page} is outside the resolver capacity")]
    PageOutOfRange {
        /// Rejected compact page index.
        page: u32,
    },
    /// Delayed work targets an older incarnation of a recycled page.
    #[error("resident picking page generation is stale")]
    StaleGeneration,
    /// An upload completion did not target a requested page.
    #[error("resident picking page is not awaiting completion")]
    PageNotPending,
    /// A token targets an absent or not-yet-ready page.
    #[error("resident picking page is not ready")]
    PageNotResident,
    /// The local row lies outside the page's declared chunk span.
    #[error("local row {row} is outside resident page {page} with {row_count} rows")]
    LocalRowOutsidePage {
        /// Addressed resident page.
        page: u32,
        /// Rejected chunk-local row.
        row: u32,
        /// Number of valid local rows.
        row_count: u32,
    },
    /// The empty attachment token has no global identity.
    #[error("empty GPU picking token has no identity")]
    EmptyToken,
    /// A slot cannot be reused without wrapping its generation.
    #[error("resident picking page generation is exhausted")]
    GenerationExhausted,
}

/// Rejects a local row that lies past the end of its page.
pub(super) fn validate_local(
    page: ResidentPage,
    row: LocalRow,
    span: ChunkSpan,
) -> Result<(), PickingError> {
    if row.get() >= span.row_count() {
        return Err(local_error(page, row, span));
    }
    Ok(())
}

pub(super) fn local_error(page: ResidentPage, row: LocalRow, span: ChunkSpan) -> PickingError {
    PickingError::LocalRowOutsidePage {
        page: page.get(),
        row: row.get(),
        row_count: span.row_count(),
    }
}

pub(super) fn page_error(page: ResidentPage) -> PickingError {
    PickingError::PageOutOfRange { page: page.get() }
}
