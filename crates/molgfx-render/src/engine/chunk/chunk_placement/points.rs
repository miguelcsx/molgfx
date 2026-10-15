use super::{ChunkPlacementError, ChunkPlacementId, ChunkRepresentation};
use molgfx_core::ResidencyTicket;
use molgfx_math::{Mat4, Rgba8};

/// Declarative rendering of one resident generic point chunk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointChunkPlacement {
    id: ChunkPlacementId,
    ticket: ResidencyTicket,
    model_to_world: Mat4,
    diameter_pixels: f32,
    color: Rgba8,
}

impl PointChunkPlacement {
    /// Creates a generic point placement without copying its payload.
    ///
    /// # Errors
    ///
    /// Rejects non-finite transforms or invalid point diameters.
    pub fn new(
        id: ChunkPlacementId,
        ticket: ResidencyTicket,
        model_to_world: Mat4,
        diameter_pixels: f32,
        color: Rgba8,
    ) -> Result<Self, ChunkPlacementError> {
        if !model_to_world.is_finite() {
            return Err(ChunkPlacementError::NonFiniteTransform);
        }
        ChunkRepresentation::points(diameter_pixels, color)?;
        Ok(Self {
            id,
            ticket,
            model_to_world,
            diameter_pixels,
            color,
        })
    }

    /// Returns the stable caller-owned placement identity.
    #[must_use]
    pub const fn id(self) -> ChunkPlacementId {
        self.id
    }

    /// Returns the exact generic point generation to draw.
    #[must_use]
    pub const fn ticket(self) -> ResidencyTicket {
        self.ticket
    }

    /// Returns the dataset-local to world-space transform.
    #[must_use]
    pub const fn model_to_world(self) -> Mat4 {
        self.model_to_world
    }

    /// Returns the validated point representation.
    #[must_use]
    pub const fn representation(self) -> ChunkRepresentation {
        ChunkRepresentation::Points {
            diameter_pixels: self.diameter_pixels,
            color: self.color,
        }
    }

    /// Returns the physical-pixel diameter.
    #[must_use]
    pub const fn diameter_pixels(self) -> f32 {
        self.diameter_pixels
    }

    /// Returns the uniform packed display color.
    #[must_use]
    pub const fn color(self) -> Rgba8 {
        self.color
    }
}
