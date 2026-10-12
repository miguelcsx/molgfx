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
    /// A word `MolFrame` does not define but the command language accepts as
    /// an alias (`ligands`, `resi`, `b_factor`; see
    /// [`registry::KEYWORD_ALIASES`](crate::registry::KEYWORD_ALIASES)) is
    /// rewritten to its canonical spelling before compiling. The query keeps
    /// the canonical text, so its fingerprint does not depend on the alias.
    ///
    /// # Errors
    ///
    /// Returns `MolFrame`'s diagnostics, located within `source` as written.
    pub fn compile(source: &str) -> Result<Self, Vec<molframe::Diagnostic>> {
        let original = match molframe::Query::compile(source) {
            Ok(query) => return Ok(Self::new(query)),
            Err(findings) => findings,
        };
        let mut rewritten = source.to_owned();
        let mut failure = original.clone();
        // Each pass fixes one alias, so a query with several needs several; the
        // bound keeps a pathological input from looping.
        for _ in 0..32 {
            let Some(next) = rewrite_alias(&rewritten, &failure) else {
                break;
            };
            rewritten = next;
            match molframe::Query::compile(&rewritten) {
                Ok(query) => return Ok(Self::new(query)),
                Err(next_failure) => failure = next_failure,
            }
        }
        Err(original)
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

/// `source` with the unknown word the first finding names replaced by its
/// canonical spelling, when that word is a known alias.
fn rewrite_alias(source: &str, findings: &[molframe::Diagnostic]) -> Option<String> {
    let first = findings.first()?;
    let item = first
        .context()
        .iter()
        .find(|item| matches!(item.label(), "keyword" | "property" | "same key"))?;
    let canonical = crate::registry::canonical_keyword(item.value())?;
    let span = first.span()?;
    let start = usize::try_from(span.start.byte_offset).ok()?;
    let end = usize::try_from(span.end).ok()?;
    let found = source.get(start..end)?;
    // The span may cover the word and what follows it; replace the word only.
    let offset = found
        .to_ascii_lowercase()
        .find(&item.value().to_ascii_lowercase())?;
    let from = start + offset;
    let to = from + item.value().len();
    let mut next = String::with_capacity(source.len() + canonical.len());
    next.push_str(source.get(..from)?);
    next.push_str(canonical);
    next.push_str(source.get(to..)?);
    Some(next)
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
