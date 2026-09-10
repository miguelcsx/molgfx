//! Typed failures produced before GPU records are emitted.

use pdviewx_core::EntityIdError;
use std::fmt;

/// A source row or compacted offset cannot be represented without aliasing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PackingError {
    /// A picking identity exceeded the chunk-local attachment limit.
    Entity(EntityIdError),
    /// A CPU offset exceeded the GPU index width.
    IndexOverflow {
        /// Table or buffer being indexed.
        resource: &'static str,
        /// Rejected offset.
        index: u64,
    },
}

impl From<EntityIdError> for PackingError {
    fn from(error: EntityIdError) -> Self {
        Self::Entity(error)
    }
}

impl fmt::Display for PackingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Entity(error) => error.fmt(formatter),
            Self::IndexOverflow { resource, index } => {
                write!(
                    formatter,
                    "{resource} index {index} exceeds the GPU u32 limit"
                )
            }
        }
    }
}

impl std::error::Error for PackingError {}
