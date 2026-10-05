//! Typed failures from source-backed cartoon geometry.

use crate::PackingError;
use std::fmt;

/// Cartoon geometry could not preserve its source anchors or GPU layout.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CartoonError {
    /// Geometry cannot fit the compact GPU layout.
    Packing(PackingError),
    /// A selected isolated guide has no source atoms defining its direction.
    GuideDirection {
        /// Encoded source guide identity.
        entity: u32,
    },
    /// Direction wedges require a linear polymer guide trace.
    DirectionProfile,
    /// Gap geometry cannot resolve finite, distinct dash positions.
    GapResolution,
}

impl From<PackingError> for CartoonError {
    fn from(error: PackingError) -> Self {
        Self::Packing(error)
    }
}

impl fmt::Display for CartoonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Packing(error) => error.fmt(formatter),
            Self::GuideDirection { entity } => {
                write!(formatter, "polymer guide {entity} has no source direction")
            }
            Self::GapResolution => formatter.write_str("polymer gap exceeds coordinate resolution"),
            Self::DirectionProfile => {
                formatter.write_str("direction wedges require a linear polymer backbone")
            }
        }
    }
}

impl std::error::Error for CartoonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Packing(error) => Some(error),
            Self::GuideDirection { .. } | Self::DirectionProfile | Self::GapResolution => None,
        }
    }
}
