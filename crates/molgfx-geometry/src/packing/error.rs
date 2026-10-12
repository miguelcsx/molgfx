//! Typed failures produced before GPU records are emitted.

use molgfx_core::EntityIdError;
use std::fmt;

/// A source row or compacted offset cannot be represented without aliasing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PackingError {
    /// Molecular topology cannot answer a requested geometry query.
    Topology {
        /// Requested topology query or invalid geometry control.
        operation: &'static str,
    },
    /// A spatial hierarchy could not represent the molecular source.
    Hierarchy(molgfx_math::BvhBuildError),
    /// Derived geometry exceeded an explicit bounded work budget.
    GeometryBudget {
        /// Resource whose complete generation was refused.
        resource: &'static str,
    },
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
            Self::Topology { operation } => {
                write!(formatter, "molecular topology cannot resolve {operation}")
            }
            Self::Entity(error) => error.fmt(formatter),
            Self::Hierarchy(error) => error.fmt(formatter),
            Self::GeometryBudget { resource } => write!(
                formatter,
                "cannot generate complete {resource} within the geometry budget"
            ),
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
