use super::ChunkPlacementId;
use pdviewx_core::{AnalyticTemplate, ResidencyTicket};
use pdviewx_math::Rgba8;
use std::sync::Arc;

/// Declarative analytic rendering of one resident rigid-instance chunk.
#[derive(Clone, Debug)]
pub struct InstanceChunkPlacement {
    id: ChunkPlacementId,
    ticket: ResidencyTicket,
    template: Arc<AnalyticTemplate>,
    color: Rgba8,
}

impl PartialEq for InstanceChunkPlacement {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.ticket == other.ticket
            && Arc::ptr_eq(&self.template, &other.template)
            && self.color == other.color
    }
}

impl InstanceChunkPlacement {
    /// References immutable template geometry and one exact transform generation.
    #[must_use]
    pub fn new(
        id: ChunkPlacementId,
        ticket: ResidencyTicket,
        template: Arc<AnalyticTemplate>,
        color: Rgba8,
    ) -> Self {
        Self {
            id,
            ticket,
            template,
            color,
        }
    }

    /// Stable caller-owned placement identity.
    #[must_use]
    pub const fn id(&self) -> ChunkPlacementId {
        self.id
    }

    /// Exact rigid-transform generation.
    #[must_use]
    pub const fn ticket(&self) -> ResidencyTicket {
        self.ticket
    }

    /// Shared homogeneous analytic template.
    #[must_use]
    pub const fn template(&self) -> &Arc<AnalyticTemplate> {
        &self.template
    }

    /// Uniform fallback color used until a paged visual descriptor is attached.
    #[must_use]
    pub const fn color(&self) -> Rgba8 {
        self.color
    }
}
