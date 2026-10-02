//! What one executed command reports.

use molgfx_scene::ScenePatch;
use serde::Serialize;

/// What executing a program did.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Outcome {
    /// The patch applied to the scene, for a viewer to apply in turn; `None`
    /// when only the session's names changed.
    pub patch: Option<ScenePatch>,
    /// The scene revision afterwards.
    pub revision: u64,
    /// Notes for the author, such as a reused layer.
    pub messages: Vec<String>,
}
