//! Anisotropic-displacement ellipsoid overlays.

use crate::Color;
use crate::id::StructureId;
use crate::representation::Selection;
use serde::{Deserialize, Serialize};

/// Per-atom anisotropic-displacement ellipsoid overlay.
///
/// Every selected atom that carries a displacement tensor in its source is
/// drawn as one ellipsoid, so the item is a selection plus a display style
/// rather than a per-atom list. Atoms without a tensor are simply skipped.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct EllipsoidSpec {
    /// Structure whose per-atom tensors the selection reads.
    pub structure: StructureId,
    /// Atoms to draw; only those carrying a tensor produce geometry.
    pub selection: Selection,
    /// Multiplier on the displacement tensor.
    ///
    /// The tensor is a mean-square displacement, so the drawn surface is one
    /// standard deviation; a larger scale widens the ellipsoid by that factor.
    /// Applied as `scale² · U`, since lengths scale with the square root of a
    /// displacement tensor's eigenvalues.
    #[serde(default = "default_ellipsoid_scale")]
    pub scale: f32,
    /// Display color.
    pub color: Color,
    /// Final opacity in `[0, 1]`.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// The one-standard-deviation surface an unscaled tensor already describes.
pub(super) fn default_ellipsoid_scale() -> f32 {
    1.0
}

pub(super) fn default_opacity() -> f32 {
    1.0
}
