//! A live authoring session over one scene.
//!
//! The session owns names — structures, named selections and layers — plus
//! the colour rules and focus it created and the history of what it did. It
//! does not own the scene: every call borrows the scene it edits, so a
//! notebook, a viewer and the session all share one scene and one revision
//! stream. When the scene is edited elsewhere the session notices the moved
//! revision, forgets whatever vanished, and drops its history, because its
//! inverses were computed against a scene that no longer exists.

#[cfg(test)]
#[path = "auto_tests.rs"]
mod auto_tests;
mod complete;
mod explain;
mod history;
#[cfg(test)]
mod overlay_tests;
mod plan;
#[cfg(test)]
mod pocket_tests;
mod property;
mod resolve;
#[cfg(test)]
mod selection_tests;
mod state;
#[cfg(test)]
mod tests;

pub use complete::Completion;
pub use state::{LayerSpec, RuleSpec, SessionSpec};

mod authoring;
mod label;
mod outcome;

pub use authoring::Session;
use label::label;
pub use outcome::Outcome;
