//! Cached canonical molecular selection value.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

/// A canonical `MolFrame` query carried by a declarative specification.
#[derive(Debug)]
pub struct Selection {
    source: Box<str>,
    fingerprint: OnceLock<Result<molframe::QueryFingerprint, Box<str>>>,
}

impl Selection {
    /// Canonical `MolFrame` query text.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Canonical fingerprint of `MolFrame`'s normalized query plan.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the query cannot compile.
    pub fn fingerprint(&self) -> Result<molframe::QueryFingerprint, crate::Error> {
        self.fingerprint
            .get_or_init(|| {
                molframe::Query::compile(&self.source)
                    .map(|query| query.fingerprint())
                    .map_err(|diagnostics| format!("{diagnostics:?}").into_boxed_str())
            })
            .clone()
            .map_err(|diagnostics| {
                crate::Error::InvalidSpec(format!("selection diagnostics: {diagnostics}"))
            })
    }

    /// Stable textual cache key for the normalized query plan.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the query cannot compile.
    pub fn stable_hash(&self) -> Result<String, crate::Error> {
        Ok(self.fingerprint()?.to_string())
    }

    /// Deterministic explanation of the canonical query identity.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the query cannot compile.
    pub fn explain(&self) -> Result<String, crate::Error> {
        Ok(format!(
            "Selection\nsource: {}\nhash: {}",
            self.source,
            self.stable_hash()?
        ))
    }

    fn from_query(query: &molframe::Query) -> Self {
        let fingerprint = OnceLock::new();
        let _already_initialized = fingerprint.set(Ok(query.fingerprint()));
        Self {
            source: query.source().into(),
            fingerprint,
        }
    }
}

impl Clone for Selection {
    fn clone(&self) -> Self {
        let fingerprint = OnceLock::new();
        if let Some(value) = self.fingerprint.get() {
            let _already_initialized = fingerprint.set(value.clone());
        }
        Self {
            source: self.source.clone(),
            fingerprint,
        }
    }
}

impl PartialEq for Selection {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl Eq for Selection {}

impl PartialOrd for Selection {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Selection {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.source.cmp(&other.source)
    }
}

impl Hash for Selection {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
    }
}

impl Serialize for Selection {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.source)
    }
}

impl<'de> Deserialize<'de> for Selection {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::from)
    }
}

impl From<&str> for Selection {
    fn from(source: &str) -> Self {
        Self {
            source: source.into(),
            fingerprint: OnceLock::new(),
        }
    }
}

impl From<String> for Selection {
    fn from(source: String) -> Self {
        Self {
            source: source.into_boxed_str(),
            fingerprint: OnceLock::new(),
        }
    }
}

impl From<molframe::Query> for Selection {
    fn from(query: molframe::Query) -> Self {
        Self::from_query(&query)
    }
}

impl From<molframe::query::Builder> for Selection {
    fn from(builder: molframe::query::Builder) -> Self {
        let query = molframe::Query::from(builder);
        Self::from_query(&query)
    }
}
