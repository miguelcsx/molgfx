use super::{ChunkPlacementId, ChunkRepresentation};
use molgfx_core::ResidencyTicket;
use molgfx_math::Mat4;

/// One transform and representation referencing a generation-bearing global ticket.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StructureChunkPlacement {
    /// Stable placement identity.
    pub id: ChunkPlacementId,
    /// Exact resident chunk generation to draw.
    pub ticket: ResidencyTicket,
    /// Dataset-local coordinates to world space.
    pub model_to_world: Mat4,
    /// Declarative rendering recipe.
    pub representation: ChunkRepresentation,
}
