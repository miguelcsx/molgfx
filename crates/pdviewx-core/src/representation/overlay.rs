//! Depth-independent screen overlays composed after molecular presentation.

use pdviewx_math::Rgba8;

/// Anchor in normalized target coordinates plus a physical-pixel offset.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct OverlayAnchor {
    /// Normalized target coordinate from bottom-left.
    pub normalized: [f32; 2],
    /// Additional offset in physical pixels.
    pub pixels: [f32; 2],
}

impl OverlayAnchor {
    /// Validates an anchor.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::InvalidOverlay`] for non-finite values or
    /// normalized coordinates outside `[0, 1]`.
    pub fn new(normalized: [f32; 2], pixels: [f32; 2]) -> Result<Self, crate::CoreError> {
        if !normalized.into_iter().chain(pixels).all(f32::is_finite)
            || normalized
                .into_iter()
                .any(|value| !(0.0..=1.0).contains(&value))
        {
            return Err(crate::CoreError::InvalidOverlay {
                reason: "overlay anchors must be finite and normalized coordinates need [0, 1]",
            });
        }
        Ok(Self { normalized, pixels })
    }
}

/// Typed content of one screen overlay.
#[derive(Clone, PartialEq, Debug)]
pub enum OverlayContent {
    /// Caller-authored text.
    Text {
        /// UTF-8 content rendered by the shared label atlas.
        text: String,
        /// Text colour.
        color: Rgba8,
        /// Physical-pixel cap height.
        size_pixels: f32,
    },
    /// Scalar colour key.
    ColorLegend {
        /// Display title.
        title: String,
        /// Inclusive scalar endpoints.
        range: [f32; 2],
        /// Endpoint colours.
        colors: [Rgba8; 2],
        /// Physical-pixel rectangle size.
        size_pixels: [f32; 2],
    },
    /// Camera-aware molecular scale bar.
    ScaleBar {
        /// Represented world length in ångström.
        length_angstrom: f32,
        /// Bar colour.
        color: Rgba8,
        /// Physical-pixel line width.
        width_pixels: f32,
    },
    /// View-coordinate XYZ tripod.
    CoordinateTripod {
        /// Axis length in physical pixels.
        size_pixels: f32,
        /// Axis width in physical pixels.
        width_pixels: f32,
    },
}

/// One persistent depth-independent overlay.
#[derive(Clone, PartialEq, Debug)]
pub struct ScreenOverlay {
    content: OverlayContent,
    anchor: OverlayAnchor,
    order: i16,
    visible: bool,
}

impl ScreenOverlay {
    /// Creates a visible overlay after validating size and range fields.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::InvalidOverlay`] for malformed content.
    pub fn new(content: OverlayContent, anchor: OverlayAnchor) -> Result<Self, crate::CoreError> {
        validate_content(&content)?;
        Ok(Self {
            content,
            anchor,
            order: 0,
            visible: true,
        })
    }

    /// Typed overlay payload.
    #[must_use]
    pub const fn content(&self) -> &OverlayContent {
        &self.content
    }

    /// Replaces typed content after validation.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::InvalidOverlay`] for malformed content.
    pub fn set_content(&mut self, content: OverlayContent) -> Result<(), crate::CoreError> {
        validate_content(&content)?;
        self.content = content;
        Ok(())
    }

    /// Screen anchor.
    #[must_use]
    pub const fn anchor(&self) -> OverlayAnchor {
        self.anchor
    }

    /// Replaces the screen anchor.
    pub const fn set_anchor(&mut self, anchor: OverlayAnchor) {
        self.anchor = anchor;
    }

    /// Stable composition order.
    #[must_use]
    pub const fn order(&self) -> i16 {
        self.order
    }

    /// Replaces composition order; lower values draw first.
    pub const fn set_order(&mut self, order: i16) {
        self.order = order;
    }

    /// Whether the overlay draws.
    #[must_use]
    pub const fn visible(&self) -> bool {
        self.visible
    }

    /// Shows or hides the overlay.
    pub const fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

fn validate_content(content: &OverlayContent) -> Result<(), crate::CoreError> {
    let valid = match content {
        OverlayContent::Text { size_pixels, .. } => size_pixels.is_finite() && *size_pixels > 0.0,
        OverlayContent::ColorLegend {
            range, size_pixels, ..
        } => {
            range.iter().all(|value| value.is_finite())
                && range[0] < range[1]
                && size_pixels
                    .iter()
                    .all(|value| value.is_finite() && *value > 0.0)
        }
        OverlayContent::ScaleBar {
            length_angstrom,
            width_pixels,
            ..
        } => {
            length_angstrom.is_finite()
                && *length_angstrom > 0.0
                && width_pixels.is_finite()
                && *width_pixels > 0.0
        }
        OverlayContent::CoordinateTripod {
            size_pixels,
            width_pixels,
        } => {
            size_pixels.is_finite()
                && *size_pixels > 0.0
                && width_pixels.is_finite()
                && *width_pixels > 0.0
        }
    };
    valid.then_some(()).ok_or(crate::CoreError::InvalidOverlay {
        reason: "overlay sizes and ranges must be finite, positive, and ordered",
    })
}
