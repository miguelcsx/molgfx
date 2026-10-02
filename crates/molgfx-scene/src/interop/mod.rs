//! `MolViewSpec` v1 adapter around the stable [`crate::SceneSpec`] contract.

#[cfg(test)]
mod tests;

mod import;
mod schema;
#[cfg(test)]
mod schema_tests;
mod snapshot;

mod codec;
mod diagnostic;
mod document;
mod export;

pub use codec::{from_mvsj, from_mvsx, to_mvsj, to_mvsx};
pub use diagnostic::Diagnostic;
pub use document::{MvsDocument, MvsImport, MvsNode};
use export::export_document;
pub use snapshot::SceneSnapshot;
