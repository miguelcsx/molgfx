//! Complete render-session persistence for reproducible figures.
//!
//! Core scene manifests intentionally remain independent from GPU presentation
//! state. This envelope joins the scene description to the camera and ordered
//! render profile without copying caller-owned structure or volume data.

use super::RenderProfile;
use crate::error::RenderError;
use pdviewx_core::{Scene, SceneDescription, SceneDescriptionSources};
use pdviewx_math::Camera;
use serde::{Deserialize, Serialize};

/// A reproducible figure/session envelope.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RenderSession {
    /// Schema version of this presentation envelope.
    pub schema: u16,
    /// Core scene composition and source fingerprints.
    pub scene: SceneDescription,
    /// Camera state used for the frame.
    pub camera: Camera,
    /// Ordered, weighted presentation modules.
    pub profile: RenderProfile,
}

impl RenderSession {
    /// Current session-envelope schema.
    pub const SCHEMA: u16 = 1;

    /// Captures scene composition without copying caller-owned source data.
    #[must_use]
    pub fn new(scene: &Scene, camera: Camera, profile: RenderProfile) -> Self {
        Self {
            schema: Self::SCHEMA,
            scene: scene.describe(),
            camera,
            profile,
        }
    }

    /// Encodes the complete session as stable pretty JSON.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::SessionEncoding`] when serialization fails.
    pub fn to_json(&self) -> Result<String, RenderError> {
        serde_json::to_string_pretty(self).map_err(|error| RenderError::SessionEncoding {
            summary: error.to_string(),
        })
    }

    /// Decodes and validates a session envelope without loading sources.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::SessionEncoding`] for malformed JSON or an
    /// unsupported session schema.
    pub fn from_json(source: &str) -> Result<Self, RenderError> {
        let session: Self =
            serde_json::from_str(source).map_err(|error| RenderError::SessionEncoding {
                summary: error.to_string(),
            })?;
        if session.schema != Self::SCHEMA {
            return Err(RenderError::SessionEncoding {
                summary: format!(
                    "unsupported render session schema; expected {}",
                    Self::SCHEMA
                ),
            });
        }
        Ok(session)
    }

    /// Rehydrates the core scene from the caller's source allocations.
    ///
    /// # Errors
    ///
    /// Returns the core validation error when a source fingerprint, handle or
    /// scene-owned payload does not match the session.
    pub fn restore(
        &self,
        sources: SceneDescriptionSources<'_>,
    ) -> Result<(Scene, Camera, RenderProfile), pdviewx_core::CoreError> {
        let scene = Scene::from_description(&self.scene, sources)?;
        Ok((scene, self.camera, self.profile.clone()))
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
