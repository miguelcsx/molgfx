//! Scene-independent visual programs bound to exact resident attribute columns.

use crate::{ResidencyTicket, VisualAttributeRef, VisualColumnKey, VisualError, VisualStyle};
use std::sync::Arc;

/// One program column key bound to an exact attribute payload generation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChunkVisualBinding {
    key: VisualColumnKey,
    ticket: ResidencyTicket,
}

impl ChunkVisualBinding {
    /// Binds one stable program key to one exact resident attribute generation.
    #[must_use]
    pub const fn new(key: VisualColumnKey, ticket: ResidencyTicket) -> Self {
        Self { key, ticket }
    }

    /// Program-visible column identity.
    #[must_use]
    pub const fn key(self) -> VisualColumnKey {
        self.key
    }

    /// Exact attribute payload generation.
    #[must_use]
    pub const fn ticket(self) -> ResidencyTicket {
        self.ticket
    }
}

/// One typed visual style plus its scene-independent immutable column bindings.
#[derive(Clone, PartialEq, Debug)]
pub struct ChunkVisualDescriptor {
    style: VisualStyle,
    bindings: Arc<[ChunkVisualBinding]>,
    order: i32,
}

impl ChunkVisualDescriptor {
    /// Validates an exact one-to-one binding for every paged program column.
    ///
    /// # Errors
    ///
    /// Rejects scene handles, legacy properties, missing, duplicate or extra
    /// column keys. Drawable compatibility is validated by the placement API.
    pub fn new(
        style: VisualStyle,
        bindings: Arc<[ChunkVisualBinding]>,
    ) -> Result<Self, VisualError> {
        let attributes = style.program().attributes();
        if attributes.len() != bindings.len()
            || bindings.iter().enumerate().any(|(index, binding)| {
                attributes
                    .get(index)
                    .and_then(|reference| reference.column())
                    != Some(binding.key)
            })
            || attributes
                .iter()
                .any(|reference| !matches!(reference, VisualAttributeRef::Column { .. }))
        {
            return Err(VisualError::InvalidColumnBinding);
        }
        Ok(Self {
            style,
            bindings,
            order: 0,
        })
    }

    /// Sets deterministic compositing order for future Studio control.
    #[must_use]
    pub const fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// Typed portable visual program and parameters.
    #[must_use]
    pub const fn style(&self) -> &VisualStyle {
        &self.style
    }

    /// Exact bindings in visual-program attribute-slot order.
    #[must_use]
    pub const fn bindings(&self) -> &Arc<[ChunkVisualBinding]> {
        &self.bindings
    }

    /// Stable caller-defined order.
    #[must_use]
    pub const fn order(&self) -> i32 {
        self.order
    }
}
