//! Structure reading for the benchmark harness.
//!
//! One parse decision for the crate: recover past a malformed row rather than
//! refusing the file, and read only the first model so a multi-model entry
//! measures the state it deposits.

use std::error::Error;
use std::io;
use std::path::Path;
use std::sync::Arc;

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

/// One model's coordinates, one triple per atom.
pub type ModelPositions = Arc<[[f32; 3]]>;

/// Reads every model of a multi-model entry as one coordinate array each.
///
/// # Errors
///
/// Returns an error when the file cannot be read even in recovery mode.
pub fn read_model_positions(path: &Path) -> Result<Vec<ModelPositions>, Box<dyn Error>> {
    let options = molframe::ReadOptions::new().mode(molframe::ParseMode::Recover);
    let (structure, _) = molframe::read_with_options(path, &options).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    // A ragged ensemble holds each model as its own structure; a dense one as
    // one coordinate block per model.
    let core = structure.engine();
    let models: Vec<ModelPositions> = match core.ragged_models() {
        Some(models) => models
            .iter()
            .map(|model| Arc::from(model.positions()))
            .collect(),
        None => (0..core.model_count())
            .filter_map(|index| u32::try_from(index).ok())
            .filter_map(|index| core.model_positions(molframe::ModelIndex::new(index)))
            .map(Arc::from)
            .collect(),
    };
    Ok(models)
}
