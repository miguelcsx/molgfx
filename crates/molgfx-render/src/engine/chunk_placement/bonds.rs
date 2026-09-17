use super::{ChunkPlacementError, ChunkPlacementId};
use molgfx_core::ResidencyTicket;
use molgfx_math::{Mat4, Rgba8};

/// Declarative analytic rendering of one resident provider bond chunk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BondChunkPlacement {
    /// Stable placement identity in the shared bounded placement table.
    pub id: ChunkPlacementId,
    /// Exact resident bond generation to draw.
    pub ticket: ResidencyTicket,
    /// Atom-dataset coordinates to world space.
    pub model_to_world: Mat4,
    /// Analytic cylinder radius in local coordinate units.
    pub radius: f32,
    /// Uniform packed display color.
    pub color: Rgba8,
}

impl BondChunkPlacement {
    /// Creates a licorice-style analytic cylinder placement.
    ///
    /// Ball-and-stick is composed by declaring this placement together with
    /// space-filling atom placements; both reference the same coordinate arena.
    ///
    /// # Errors
    ///
    /// Rejects a non-finite transform or a non-positive/non-finite radius.
    pub fn licorice(
        id: ChunkPlacementId,
        ticket: ResidencyTicket,
        model_to_world: Mat4,
        radius: f32,
        color: Rgba8,
    ) -> Result<Self, ChunkPlacementError> {
        if !model_to_world.is_finite() {
            return Err(ChunkPlacementError::NonFiniteTransform);
        }
        if !radius.is_finite() || radius <= 0.0 {
            return Err(ChunkPlacementError::InvalidBondRadius);
        }
        Ok(Self {
            id,
            ticket,
            model_to_world,
            radius,
            color,
        })
    }
}
