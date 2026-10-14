//! The `MolViewSpec` document model.

use super::Diagnostic;
use crate::SceneSpec;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Generic exhaustive `MolViewSpec` node envelope.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct MvsNode {
    /// Node kind from the v1 schema.
    pub kind: Box<str>,
    /// Kind-specific parameters, preserved even when unsupported.
    #[serde(default)]
    pub params: Map<String, Value>,
    /// Ordered children.
    #[serde(default)]
    pub children: Vec<MvsNode>,
}

/// `MolViewSpec` v1 JSON document.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct MvsDocument {
    /// Specification metadata.
    #[serde(default)]
    pub metadata: Map<String, Value>,
    /// Required root node.
    pub root: MvsNode,
}

/// Imported scene plus loss diagnostics.
#[derive(Clone, PartialEq, Debug)]
pub struct MvsImport {
    /// Portable `MolGFX` semantic scene.
    pub scene: SceneSpec,
    /// Unsupported or approximated source information.
    pub diagnostics: Vec<Diagnostic>,
}
