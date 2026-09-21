//! Public authoring and patch errors.

/// Failure while building or mutating a declarative scene.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A declarative value is malformed or unsupported by its representation.
    #[error("invalid scene specification: {0}")]
    InvalidSpec(String),
    /// An identifier is absent from the current scene revision.
    #[error("scene identifier does not exist")]
    MissingId,
    /// The resolved scene rejected a semantically valid authoring operation.
    #[error(transparent)]
    Core(#[from] molgfx_core::CoreError),
    /// Renderer initialization or execution failed.
    #[error(transparent)]
    Render(#[from] molgfx_render::RenderError),
    /// A serialized scene could not be read or written.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// An incremental patch could not be applied.
    #[error(transparent)]
    Patch(#[from] PatchError),
    /// Image output could not be written.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Why an incremental scene patch was not applied.
#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
pub enum PatchError {
    /// The patch was created against another scene revision.
    #[error("patch expects revision {expected}, scene is at revision {actual}")]
    RevisionConflict {
        /// Revision declared by the patch.
        expected: u64,
        /// Current scene revision.
        actual: u64,
    },
    /// One operation names an object absent from the base scene.
    #[error("patch refers to an absent scene object")]
    MissingId,
    /// One operation contains invalid representation state.
    #[error("patch contains invalid state: {0}")]
    Invalid(String),
}
