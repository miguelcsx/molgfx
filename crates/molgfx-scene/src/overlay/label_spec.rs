//! Label and annotation specifications.

use super::Anchor;
use crate::Color;
use serde::{Deserialize, Serialize};

/// Label or annotation specification.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AnnotationSpec {
    /// Semantic placement of the label.
    pub anchor: Anchor,
    /// Display text.
    pub text: Box<str>,
    /// Label color.
    pub color: Color,
}

/// Immutable label builder.
#[derive(Clone, PartialEq, Debug)]
pub struct Label(pub(super) AnnotationSpec);

impl Label {
    /// Sets the label color.
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.0.color = color;
        self
    }
}
