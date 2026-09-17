use thiserror::Error;

/// Current draw eligibility of one declarative placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkPlacementStatus {
    /// Its exact generation is resident and enters the shared indirect batch.
    Resident,
    /// The placement remains declared but its ticket is absent or not ready.
    NotResident,
    /// No placement with this identity is declared.
    Missing,
}

/// Why a declarative placement set could not be accepted.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ChunkPlacementError {
    /// A point diameter was zero, negative or non-finite.
    #[error("point diameter must be finite and positive")]
    InvalidPointDiameter,
    /// A radius scale was zero, negative or non-finite.
    #[error("spacefill radius scale must be finite and positive")]
    InvalidRadiusScale,
    /// A bond radius was zero, negative or non-finite.
    #[error("bond radius must be finite and positive")]
    InvalidBondRadius,
    /// A relation width or opacity was outside its finite visual range.
    #[error("relation style must have positive finite width and bounded opacity")]
    InvalidRelationStyle,
    /// A paged relation visual requested an unsupported output channel.
    #[error("paged visual program is incompatible with relation glyphs")]
    InvalidRelationVisual,
    /// A transform contains a non-finite component.
    #[error("chunk placement transform must be finite")]
    NonFiniteTransform,
    /// Two placements reused the same caller identity.
    #[error("chunk placement identity {id} is duplicated")]
    DuplicateIdentity {
        /// Reused identity.
        id: u64,
    },
    /// The fixed placement working-set capacity was exhausted.
    #[error("chunk placement capacity {capacity} is exhausted")]
    Capacity {
        /// Configured maximum resident placements.
        capacity: usize,
    },
    /// A trajectory interpolation factor was not in the closed unit interval.
    #[error("trajectory interpolation must be finite and between zero and one")]
    InvalidInterpolation,
}
