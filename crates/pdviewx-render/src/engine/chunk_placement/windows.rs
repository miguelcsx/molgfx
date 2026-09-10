use super::{ChunkPlacementError, ChunkPlacementId};
use pdviewx_core::ResidencyTicket;

macro_rules! validate_interpolation {
    ($value:expr) => {
        if !$value.is_finite() || !(0.0..=1.0).contains(&$value) {
            return Err(ChunkPlacementError::InvalidInterpolation);
        }
    };
}

/// Bounded two-frame window for one topology-stable structure chunk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrajectoryChunkWindow {
    /// Exact resident structure generation receiving interpolated coordinates.
    pub structure: ResidencyTicket,
    /// Exact resident provider-frame generation at interpolation zero.
    pub start: ResidencyTicket,
    /// Exact resident provider-frame generation at interpolation one.
    pub end: ResidencyTicket,
    /// Closed-interval interpolation factor.
    pub interpolation: f32,
}

/// Bounded two-frame window for one declared rigid-instance occurrence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InstanceChunkWindow {
    /// Stable placement whose transform stream is animated.
    pub placement: ChunkPlacementId,
    /// Exact resident rigid-transform generation at interpolation zero.
    pub start: ResidencyTicket,
    /// Exact resident rigid-transform generation at interpolation one.
    pub end: ResidencyTicket,
    /// Closed-interval interpolation factor.
    pub interpolation: f32,
}

/// Bounded two-frame window for one scene-independent attribute column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttributeChunkWindow {
    /// Stable attribute ticket referenced by visual descriptors.
    pub attribute: ResidencyTicket,
    /// Exact resident attribute generation at interpolation zero.
    pub start: ResidencyTicket,
    /// Exact resident attribute generation at interpolation one.
    pub end: ResidencyTicket,
    /// Closed-interval interpolation factor.
    pub interpolation: f32,
}

impl AttributeChunkWindow {
    /// Creates a topology- and type-stable paged attribute window.
    ///
    /// # Errors
    /// Rejects interpolation outside the closed unit interval.
    pub fn new(
        attribute: ResidencyTicket,
        start: ResidencyTicket,
        end: ResidencyTicket,
        interpolation: f32,
    ) -> Result<Self, ChunkPlacementError> {
        validate_interpolation!(interpolation);
        Ok(Self {
            attribute,
            start,
            end,
            interpolation,
        })
    }
}

impl InstanceChunkWindow {
    /// Creates a topology-stable paged rigid-transform window.
    ///
    /// # Errors
    /// Rejects interpolation outside the closed unit interval.
    pub fn new(
        placement: ChunkPlacementId,
        start: ResidencyTicket,
        end: ResidencyTicket,
        interpolation: f32,
    ) -> Result<Self, ChunkPlacementError> {
        validate_interpolation!(interpolation);
        Ok(Self {
            placement,
            start,
            end,
            interpolation,
        })
    }
}

impl TrajectoryChunkWindow {
    /// Creates a topology-stable two-frame window.
    ///
    /// # Errors
    /// Rejects interpolation outside the closed unit interval.
    pub fn new(
        structure: ResidencyTicket,
        start: ResidencyTicket,
        end: ResidencyTicket,
        interpolation: f32,
    ) -> Result<Self, ChunkPlacementError> {
        validate_interpolation!(interpolation);
        Ok(Self {
            structure,
            start,
            end,
            interpolation,
        })
    }
}
