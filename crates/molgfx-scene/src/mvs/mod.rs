//! `MolViewSpec` v1 adapter around the stable [`crate::SceneSpec`] contract.
//!
//! The one place that names the `MolViewSpec` standard. It reads and writes the
//! compatible subset and reports everything else as a [`Diagnostic`], so a node
//! the standard adds later is a diagnostic, not a failure, and nothing else in the
//! engine depends on this module.

#[cfg(test)]
mod tests;

mod import;
mod schema;
#[cfg(test)]
mod schema_tests;

mod codec;
mod diagnostic;
mod document;
mod export;

pub use codec::{from_mvsj, to_mvsj};
#[cfg(feature = "mvsx")]
pub use codec::{from_mvsx, to_mvsx};
pub use diagnostic::Diagnostic;
pub use document::{MvsDocument, MvsImport, MvsNode};
use export::export_document;
