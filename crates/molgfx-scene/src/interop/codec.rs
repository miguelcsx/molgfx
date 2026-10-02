//! Reading and writing `MolViewSpec` as JSON and archives.

use super::{MvsDocument, MvsImport, export_document, import};
use crate::SceneSpec;
use std::io::{Cursor, Read, Write};

/// Exports the compatible semantic subset as deterministic `.mvsj` JSON.
///
/// # Errors
///
/// Returns an encoding error if the semantic document cannot be serialized.
pub fn to_mvsj(scene: &SceneSpec) -> Result<String, crate::Error> {
    serde_json::to_string(&export_document(scene)).map_err(crate::Error::from)
}

/// Imports an `.mvsj`, preserving unsupported data in diagnostics.
///
/// # Errors
///
/// Returns an error for malformed JSON or a document without a root node.
pub fn from_mvsj(source: &str) -> Result<MvsImport, crate::Error> {
    let document: MvsDocument = serde_json::from_str(source)?;
    import::import_document(&document)
}

/// Encodes a standard ZIP-based `.mvsx` container with `index.mvsj`.
///
/// # Errors
///
/// Returns an encoding or archive error if the container cannot be written.
pub fn to_mvsx(scene: &SceneSpec) -> Result<Vec<u8>, crate::Error> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    archive
        .start_file("index.mvsj", zip::write::SimpleFileOptions::default())
        .map_err(|error| crate::Error::InvalidSpec(error.to_string()))?;
    archive
        .write_all(to_mvsj(scene)?.as_bytes())
        .map_err(crate::Error::from)?;
    archive
        .finish()
        .map(Cursor::into_inner)
        .map_err(|error| crate::Error::InvalidSpec(error.to_string()))
}

/// Imports `index.mvsj` from a standard `.mvsx` ZIP container.
///
/// # Errors
///
/// Returns an error for an invalid archive, missing index, or malformed JSON.
pub fn from_mvsx(bytes: &[u8]) -> Result<MvsImport, crate::Error> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| crate::Error::InvalidSpec(error.to_string()))?;
    let mut source = String::new();
    archive
        .by_name("index.mvsj")
        .map_err(|error| crate::Error::InvalidSpec(error.to_string()))?
        .read_to_string(&mut source)
        .map_err(crate::Error::from)?;
    from_mvsj(&source)
}
