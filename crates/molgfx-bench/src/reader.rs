//! Structure reading for the benchmark harness.
//!
//! One parse decision for the crate: recover past a malformed row rather than
//! refusing the file, and read only the first model so a multi-model entry
//! measures the state it deposits.

use std::error::Error;
use std::io;
use std::path::Path;

/// Reads one structure, recovering past diagnostics.
///
/// Diagnostics come back already stringified so the caller reports them in
/// whatever form its own output wants.
///
/// # Errors
///
/// Returns an error when the file cannot be read even in recovery mode.
pub fn read_structure(
    path: &Path,
) -> Result<(molframe::Structure, Option<String>), Box<dyn Error>> {
    let options = molframe::ReadOptions::new()
        .mode(molframe::ParseMode::Recover)
        .only_first_model(true);
    molframe::read_with_options(path, &options)
        .map(|(structure, diagnostics)| {
            let diagnostics = if diagnostics.is_empty() {
                None
            } else {
                Some(format!("{diagnostics:?}"))
            };
            (structure, diagnostics)
        })
        .map_err(|diagnostics| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("structure diagnostics: {diagnostics:?}"),
            )
            .into()
        })
}
