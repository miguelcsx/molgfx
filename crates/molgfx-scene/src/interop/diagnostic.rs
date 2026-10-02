//! Loss reports from interchange.

use serde::{Deserialize, Serialize};

/// Information that could not be represented exactly during interchange.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable machine-readable code.
    pub code: Box<str>,
    /// Human-readable explanation.
    pub message: Box<str>,
    /// Tree path of the affected `MolViewSpec` node.
    pub path: Box<str>,
}
