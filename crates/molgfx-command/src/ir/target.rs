//! What a command acts on: a visual layer or a molecular query.

use super::name::Name;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::sync::Arc;

/// A `MolFrame` query as the author wrote it, compiled once.
///
/// It may refer to named selections with `$name`; those references are kept,
/// and resolved each time the session needs the atoms, so a layer drawn from
/// `$pocket` follows every later redefinition of `pocket`.
#[derive(Clone)]
pub struct QueryText(Arc<molframe::Query>);

impl QueryText {
    /// Wraps an already compiled query without recompiling it.
    #[must_use]
    pub fn new(query: molframe::Query) -> Self {
        Self(Arc::new(query))
    }

    /// Compiles `source` with `MolFrame`.
    ///
    /// # Errors
    ///
    /// Returns `MolFrame`'s diagnostics, located within `source`.
    pub fn compile(source: &str) -> Result<Self, Vec<molframe::Diagnostic>> {
        molframe::Query::compile(source).map(Self::new)
    }

    /// The query's canonical text.
    #[must_use]
    pub fn source(&self) -> &str {
        self.0.source()
    }

    /// The compiled query.
    #[must_use]
    pub fn query(&self) -> &molframe::Query {
        &self.0
    }

    /// Stable identity of the normalized query, references included.
    #[must_use]
    pub fn fingerprint(&self) -> molframe::QueryFingerprint {
        self.0.fingerprint()
    }

    /// Named selections the query refers to directly.
    #[must_use]
    pub fn references(&self) -> Vec<&str> {
        self.0.references()
    }
}

impl PartialEq for QueryText {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint() == other.fingerprint()
    }
}

impl fmt::Debug for QueryText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "QueryText({:?})", self.source())
    }
}

impl fmt::Display for QueryText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.source())
    }
}

impl Serialize for QueryText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.source())
    }
}

impl<'de> Deserialize<'de> for QueryText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let source = String::deserialize(deserializer)?;
        Self::compile(&source).map_err(|diagnostics| {
            serde::de::Error::custom(format!(
                "invalid MolFrame query {source:?}: {diagnostics:?}"
            ))
        })
    }
}

/// What a command acts on.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// A named visual layer, written `@name`.
    Layer(Name),
    /// A molecular query, possibly referring to named selections.
    Query(QueryText),
}

impl fmt::Display for Target {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layer(name) => write!(formatter, "@{name}"),
            Self::Query(query) => formatter.write_str(query.source()),
        }
    }
}
