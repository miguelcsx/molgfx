//! Portable origin of bulk data stored outside the scene specification.

use serde::{Deserialize, Serialize};

/// Portable origin for bulk data stored outside [`crate::SceneSpec`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct DataSource {
    /// Stable content identity; empty only while a runtime-only binding is unresolved.
    pub content_hash: Box<str>,
    /// Optional portable location.
    pub uri: Option<Box<str>>,
    /// Optional format hint.
    pub format: Option<Box<str>>,
}

impl DataSource {
    /// Describes content-addressed data with an optional portable location.
    #[must_use]
    pub fn new(content_hash: impl Into<Box<str>>) -> Self {
        Self {
            content_hash: content_hash.into(),
            uri: None,
            format: None,
        }
    }

    /// Sets a portable URI without loading data.
    #[must_use]
    pub fn uri(mut self, uri: impl Into<Box<str>>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    /// Sets a format hint.
    #[must_use]
    pub fn format(mut self, format: impl Into<Box<str>>) -> Self {
        self.format = Some(format.into());
        self
    }
}
