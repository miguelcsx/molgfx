//! The error type for scene construction and editing.
//!
//! Every variant carries a stable registry code so callers can match on the
//! condition across versions without parsing message text.

use thiserror::Error;

/// Everything that can go wrong building or editing a scene.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreError {
    /// The source structure failed to parse.
    #[error("structure could not be read: {summary}")]
    StructureRead {
        /// A one-line rendering of the parser's findings.
        summary: String,
    },

    /// A selection referenced an annotation the structure does not carry.
    #[error("selection references absent annotation {name:?}")]
    AbsentAnnotation {
        /// The annotation the selection asked for.
        name: String,
    },

    /// A representation was applied to an empty or invalid selection.
    #[error("representation applied to an empty selection")]
    EmptySelection,

    /// A handle referred to an entry that no longer exists.
    #[error("stale handle: the referenced entry was removed")]
    StaleHandle,

    /// The requested representation kind is not implemented yet.
    #[error("representation kind {kind:?} is not available yet")]
    Unsupported {
        /// The kind that was requested.
        kind: crate::representation::RepresentationKind,
    },
}

impl CoreError {
    /// The stable registry code for this condition.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::StructureRead { .. } => "PDVIEWX-E0030",
            Self::AbsentAnnotation { .. } => "PDVIEWX-E0031",
            Self::EmptySelection => "PDVIEWX-E0040",
            Self::StaleHandle => "PDVIEWX-E0041",
            Self::Unsupported { .. } => "PDVIEWX-E0042",
        }
    }
}
