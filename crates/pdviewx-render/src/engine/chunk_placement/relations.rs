use super::{ChunkPlacementError, ChunkPlacementId};
use pdviewx_core::{ChunkVisualDescriptor, RelationStyle, ResidencyTicket, VisualCompatibility};
use std::sync::Arc;

/// Declarative rendering of one globally anchored resident relation chunk.
#[derive(Clone, Debug, PartialEq)]
pub struct RelationChunkPlacement {
    id: ChunkPlacementId,
    ticket: ResidencyTicket,
    style: RelationStyle,
    visual: Option<Arc<ChunkVisualDescriptor>>,
}

impl RelationChunkPlacement {
    /// Creates one relation occurrence with a purely visual fallback style.
    ///
    /// # Errors
    ///
    /// Rejects non-positive widths and opacity outside the closed unit range.
    pub fn new(
        id: ChunkPlacementId,
        ticket: ResidencyTicket,
        style: RelationStyle,
    ) -> Result<Self, ChunkPlacementError> {
        if !style.width_pixels.is_finite()
            || style.width_pixels <= 0.0
            || !style.opacity.is_finite()
            || !(0.0..=1.0).contains(&style.opacity)
            || style
                .endpoint_insets_pixels
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(ChunkPlacementError::InvalidRelationStyle);
        }
        Ok(Self {
            id,
            ticket,
            style,
            visual: None,
        })
    }

    /// Attaches a typed scene-independent visual program and exact columns.
    ///
    /// # Errors
    ///
    /// Rejects outputs unsupported by analytic relation glyphs.
    pub fn with_visual(
        mut self,
        visual: Arc<ChunkVisualDescriptor>,
    ) -> Result<Self, ChunkPlacementError> {
        visual
            .style()
            .program()
            .validate_compatibility(VisualCompatibility::RELATIONS)
            .map_err(|_| ChunkPlacementError::InvalidRelationVisual)?;
        self.visual = Some(visual);
        Ok(self)
    }

    /// Stable caller-owned relation occurrence.
    #[must_use]
    pub const fn id(&self) -> ChunkPlacementId {
        self.id
    }

    /// Exact relation payload generation.
    #[must_use]
    pub const fn ticket(&self) -> ResidencyTicket {
        self.ticket
    }

    /// Batch-wide visual fallback used without a paged descriptor.
    #[must_use]
    pub const fn style(&self) -> RelationStyle {
        self.style
    }

    /// Optional typed visual descriptor sharing immutable program storage.
    #[must_use]
    pub const fn visual(&self) -> Option<&Arc<ChunkVisualDescriptor>> {
        self.visual.as_ref()
    }
}
