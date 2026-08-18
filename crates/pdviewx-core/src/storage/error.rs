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

    /// A caller-supplied scalar grid violates the portable volume contract.
    #[error("invalid density volume: {reason}")]
    InvalidVolume {
        /// Stable human-readable validation reason.
        reason: &'static str,
    },

    /// A clip plane or slab is malformed.
    #[error("invalid clipping configuration: {reason}")]
    InvalidClip {
        /// Stable human-readable validation reason.
        reason: &'static str,
    },

    /// A typed spatial or logical selection request is malformed.
    #[error("invalid selection: {reason}")]
    InvalidSelection {
        /// Stable human-readable validation reason.
        reason: &'static str,
    },

    /// A caller-supplied interaction record is malformed.
    #[error("invalid molecular interaction: {reason}")]
    InvalidInteraction {
        /// Stable human-readable validation reason.
        reason: &'static str,
    },

    /// Caller-supplied trajectory frames or time are malformed.
    #[error("invalid trajectory segment: {reason}")]
    InvalidTrajectory {
        /// Stable human-readable validation reason.
        reason: &'static str,
    },

    /// A caller-declared property mapping is malformed.
    #[error("invalid property mapping: {reason}")]
    InvalidProperty {
        /// Stable human-readable validation reason.
        reason: &'static str,
    },

    /// A caller-authored annotation or measurement is malformed.
    #[error("invalid annotation or measurement: {reason}")]
    InvalidAnnotation {
        /// Stable human-readable validation reason.
        reason: &'static str,
    },
    /// A caller-supplied triangle mesh is malformed.
    #[error("invalid mesh: {reason}")]
    InvalidMesh {
        /// Stable human-readable validation reason.
        reason: &'static str,
    },
    /// A weighted ensemble has malformed membership or probabilities.
    #[error("invalid ensemble: {reason}")]
    InvalidEnsemble {
        /// Stable human-readable validation reason.
        reason: &'static str,
    },
    /// A caller-supplied structure correspondence or difference style is malformed.
    #[error("invalid structural difference: {reason}")]
    InvalidDifference {
        /// Stable human-readable validation reason.
        reason: &'static str,
    },
    /// A caller-supplied categorical label volume or style table is malformed.
    #[error("invalid categorical segmentation: {reason}")]
    InvalidSegmentation {
        /// Stable human-readable validation reason.
        reason: &'static str,
    },
    /// A scene manifest could not be encoded, decoded or matched to a scene.
    #[error("invalid scene description: {summary}")]
    InvalidSceneDescription {
        /// Stable, caller-actionable diagnostic.
        summary: String,
    },
    /// A caller-supplied analytic primitive is malformed.
    #[error("invalid primitive: {reason}")]
    InvalidPrimitive {
        /// Stable validation reason.
        reason: &'static str,
    },
    /// A screen overlay carries malformed geometry or scalar limits.
    #[error("invalid screen overlay: {reason}")]
    InvalidOverlay {
        /// Stable validation reason.
        reason: &'static str,
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
            Self::InvalidVolume { .. } => "PDVIEWX-E0032",
            Self::InvalidClip { .. } => "PDVIEWX-E0033",
            Self::InvalidSelection { .. } => "PDVIEWX-E0034",
            Self::InvalidInteraction { .. } => "PDVIEWX-E0035",
            Self::InvalidTrajectory { .. } => "PDVIEWX-E0036",
            Self::InvalidProperty { .. } => "PDVIEWX-E0037",
            Self::InvalidAnnotation { .. } => "PDVIEWX-E0038",
            Self::InvalidEnsemble { .. } => "PDVIEWX-E0039",
            Self::InvalidMesh { .. } => "PDVIEWX-E0047",
            Self::EmptySelection => "PDVIEWX-E0040",
            Self::StaleHandle => "PDVIEWX-E0041",
            Self::Unsupported { .. } => "PDVIEWX-E0042",
            Self::InvalidDifference { .. } => "PDVIEWX-E0043",
            Self::InvalidSegmentation { .. } => "PDVIEWX-E0044",
            Self::InvalidSceneDescription { .. } => "PDVIEWX-E0045",
            Self::InvalidPrimitive { .. } => "PDVIEWX-E0046",
            Self::InvalidOverlay { .. } => "PDVIEWX-E0048",
        }
    }
}
