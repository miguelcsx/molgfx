//! Cached canonical molecular selection value.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

type Compiled = Result<Arc<molframe::Query>, Box<str>>;

/// A canonical `MolFrame` query carried by a declarative specification.
///
/// The text is the portable identity; the compiled query is derived from it at
/// most once per value and shared by every clone, so evaluating the same
/// selection again never re-parses it.
#[derive(Debug)]
pub struct Selection {
    source: Box<str>,
    compiled: Arc<OnceLock<Compiled>>,
}

impl Selection {
    /// Canonical `MolFrame` query text.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The compiled `MolFrame` query, compiled on first use and then shared.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the query cannot compile.
    pub fn compiled(&self) -> Result<Arc<molframe::Query>, crate::Error> {
        self.compiled
            .get_or_init(|| {
                molframe::Query::compile(&self.source)
                    .map(Arc::new)
                    .map_err(|diagnostics| format!("{diagnostics:?}").into_boxed_str())
            })
            .clone()
            .map_err(|diagnostics| {
                crate::Error::InvalidSpec(format!("selection diagnostics: {diagnostics}"))
            })
    }

    /// Canonical fingerprint of `MolFrame`'s normalized query plan.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the query cannot compile.
    pub fn fingerprint(&self) -> Result<molframe::QueryFingerprint, crate::Error> {
        Ok(self.compiled()?.fingerprint())
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

    fn from_query(query: molframe::Query) -> Self {
        let source = query.source().into();
        let compiled = OnceLock::new();
        let _already_initialized = compiled.set(Ok(Arc::new(query)));
        Self {
            source,
            compiled: Arc::new(compiled),
        }
    }

    fn from_source(source: Box<str>) -> Self {
        Self {
            source,
            compiled: Arc::new(OnceLock::new()),
        }
    }
}

impl Clone for Selection {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            compiled: Arc::clone(&self.compiled),
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
        Self::from_source(source.into())
    }
}

impl From<String> for Selection {
    fn from(source: String) -> Self {
        Self::from_source(source.into_boxed_str())
    }
}

impl From<molframe::Query> for Selection {
    fn from(query: molframe::Query) -> Self {
        Self::from_query(query)
    }
}

impl From<molframe::query::Builder> for Selection {
    fn from(builder: molframe::query::Builder) -> Self {
        Self::from_query(molframe::Query::from(builder))
    }
}
