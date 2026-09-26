//! Validated symbolic names for selections, layers and structures.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A validated authoring name.
///
/// A name starts with an ASCII letter or underscore and continues with ASCII
/// letters, digits and underscores, at most [`Name::MAX_LEN`] bytes. The same
/// spelling is what `MolFrame` accepts after `$`, so every selection name can be
/// referred to inside a query.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Name(Box<str>);

impl Name {
    /// The longest name accepted.
    pub const MAX_LEN: usize = 64;

    /// Validates `value` as a name.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidName`] describing why the spelling is not a name.
    pub fn new(value: &str) -> Result<Self, InvalidName> {
        if value.is_empty() {
            return Err(InvalidName::Empty);
        }
        if value.len() > Self::MAX_LEN {
            return Err(InvalidName::TooLong);
        }
        if !molframe::QueryAliases::is_valid_name(value) {
            return Err(InvalidName::Spelling);
        }
        Ok(Self(value.into()))
    }

    /// The name as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", self.0)
    }
}

impl TryFrom<String> for Name {
    type Error = InvalidName;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl TryFrom<&str> for Name {
    type Error = InvalidName;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Name> for String {
    fn from(value: Name) -> Self {
        value.0.into()
    }
}

/// Why a spelling is not a valid name.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum InvalidName {
    /// The name is empty.
    #[error("a name cannot be empty")]
    Empty,
    /// The name is longer than [`Name::MAX_LEN`] bytes.
    #[error("a name is at most 64 characters")]
    TooLong,
    /// The name contains a character other than a letter, digit or
    /// underscore, or starts with a digit.
    #[error("a name is a letter or underscore followed by letters, digits or underscores")]
    Spelling,
}
