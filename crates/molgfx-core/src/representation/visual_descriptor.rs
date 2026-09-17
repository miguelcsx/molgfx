//! Declarative visual attachment for one exact generic row domain.

use crate::VisualStyle;

/// One program, parameter block and stable draw order attached to a domain.
#[derive(Clone, PartialEq, Debug)]
pub struct VisualDescriptor {
    style: VisualStyle,
    order: i32,
}

impl VisualDescriptor {
    /// Creates a domain-independent descriptor validated when attached.
    #[must_use]
    pub const fn new(style: VisualStyle) -> Self {
        Self { style, order: 0 }
    }

    /// Sets deterministic compositing/draw order for future Studio control.
    #[must_use]
    pub const fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// Typed visual program and mutable parameter block.
    #[must_use]
    pub const fn style(&self) -> &VisualStyle {
        &self.style
    }

    /// Mutable parameters; the scene accessor bumps the descriptor revision.
    pub fn style_mut(&mut self) -> &mut VisualStyle {
        &mut self.style
    }

    /// Stable caller-defined order.
    #[must_use]
    pub const fn order(&self) -> i32 {
        self.order
    }
}
