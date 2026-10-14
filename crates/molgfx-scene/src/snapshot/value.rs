//! Deterministic scene snapshot transport.

use crate::SceneSpec;
use serde::{Deserialize, Serialize};

/// Complete portable semantic state captured for restoration.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SceneSnapshot {
    /// Scene state, including retained overlay descriptors and extensions.
    pub scene: SceneSpec,
    /// Stable content hash of the captured scene.
    pub content_hash: Box<str>,
}

impl SceneSnapshot {
    /// Captures a scene specification without copying runtime coordinate data.
    #[must_use]
    pub fn capture(scene: &SceneSpec) -> Self {
        Self {
            content_hash: scene.stable_hash().into(),
            scene: scene.clone(),
        }
    }

    /// Verifies the snapshot's integrity before restoration.
    ///
    /// # Errors
    ///
    /// Returns an error when the content hash, selections, overlay
    /// descriptors, or camera state is invalid.
    pub fn verify(&self) -> Result<(), crate::Error> {
        let actual = self.scene.stable_hash();
        if actual != self.content_hash.as_ref() {
            return Err(crate::Error::InvalidSpec(
                "scene snapshot content hash does not match its state".to_owned(),
            ));
        }
        self.scene.validate_selections()?;
        self.scene.validate_overlay()?;
        self.scene.validate_camera()
    }

    /// Encodes a deterministic JSON snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot fails verification.
    pub fn to_json(&self) -> Result<String, crate::Error> {
        self.verify()?;
        serde_json::to_string(self).map_err(crate::Error::from)
    }

    /// Decodes and verifies a snapshot before it can be restored.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot fails verification.
    pub fn from_json(source: &str) -> Result<Self, crate::Error> {
        let snapshot: Self = serde_json::from_str(source)?;
        snapshot.verify()?;
        Ok(snapshot)
    }
}
