//! Public, meaning-agnostic composition values.

use molgfx_core::{AttributeHandle, CoreError, RowDomain, ScalarRamp};
use molgfx_math::Rgba8;

/// Invalid generic composition or underlying scene error.
#[derive(Debug, thiserror::Error)]
pub enum CompositionError {
    /// The caller supplied a malformed visual policy.
    #[error("invalid generic composition: {0}")]
    Invalid(&'static str),
    /// The scene rejected a stale handle or incompatible attribute domain.
    #[error(transparent)]
    Core(#[from] CoreError),
    /// The bounded typed visual program was invalid.
    #[error(transparent)]
    Visual(#[from] molgfx_core::VisualError),
}

/// One domain and scalar attribute used as a normalized emphasis field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FocusLayer {
    /// Exact row table to style.
    pub domain: RowDomain,
    /// Caller-computed scalar evidence; zero is context and one is focus.
    pub emphasis: AttributeHandle,
}

/// Purely visual focus policy with no molecular distance assumptions.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FocusCompositionStyle {
    /// Opacity at zero emphasis.
    pub context_opacity: f32,
    /// Opacity at full emphasis.
    pub focus_opacity: f32,
    /// Deterministic descriptor order.
    pub order: i32,
}

impl Default for FocusCompositionStyle {
    fn default() -> Self {
        Self {
            context_opacity: 0.16,
            focus_opacity: 1.0,
            order: 0,
        }
    }
}

/// One domain and caller-computed scalar delta.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DifferenceLayer {
    /// Exact row table to style.
    pub domain: RowDomain,
    /// Scalar difference, score, uncertainty or other caller quantity.
    pub delta: AttributeHandle,
}

/// Generic difference policy expressed only as visual thresholds and a ramp.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DifferenceCompositionStyle {
    /// Values at or below this threshold remain context.
    pub context_threshold: f32,
    /// Value at which emphasis reaches full opacity.
    pub emphasis_threshold: f32,
    /// Context opacity.
    pub context_opacity: f32,
    /// Caller-selected color ramp and units/domain.
    pub ramp: ScalarRamp,
    /// First deterministic descriptor order.
    pub order: i32,
}

/// One independently rendered member of a generic weighted overlay.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EnsembleLayer {
    /// Exact row table to style.
    pub domain: RowDomain,
    /// Non-negative caller weight.
    pub weight: f32,
    /// Caller-selected visual identity; the engine owns no palette semantics.
    pub color: Rgba8,
}

/// Opacity policy for generic weighted overlays.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EnsembleCompositionStyle {
    /// Opacity of the highest-weight member.
    pub dominant_opacity: f32,
    /// Maximum opacity of every other member.
    pub alternate_opacity: f32,
    /// Discoverability floor for non-zero members.
    pub minimum_opacity: f32,
    /// First deterministic descriptor order.
    pub order: i32,
}

impl Default for EnsembleCompositionStyle {
    fn default() -> Self {
        Self {
            dominant_opacity: 1.0,
            alternate_opacity: 0.55,
            minimum_opacity: 0.08,
            order: 0,
        }
    }
}

/// Result handles for a generic composition; source data remains caller-owned.
#[derive(Clone, PartialEq, Debug)]
pub struct GenericCompositionView {
    /// Domains styled in input order.
    pub domains: Vec<RowDomain>,
    /// Normalized member weights when produced by an ensemble composition.
    pub normalized_weights: Vec<f32>,
}

/// Thin declarative compositions over generic domains.
pub trait GenericCompositionScene {
    /// Maps a caller emphasis column to focus-versus-context opacity.
    ///
    /// # Errors
    ///
    /// Returns an error for stale domains, mismatched attributes or invalid opacity bounds.
    fn compose_focus(
        &mut self,
        layer: FocusLayer,
        style: FocusCompositionStyle,
    ) -> Result<GenericCompositionView, CompositionError>;

    /// Maps caller delta columns to a shared ramp and emphasis policy.
    ///
    /// # Errors
    ///
    /// Returns an error if any layer is stale, mismatched or uses an invalid visual policy.
    fn compose_difference(
        &mut self,
        layers: &[DifferenceLayer],
        style: DifferenceCompositionStyle,
    ) -> Result<GenericCompositionView, CompositionError>;

    /// Creates a caller-colored, weight-driven overlay over arbitrary domains.
    ///
    /// # Errors
    ///
    /// Returns an error for stale or duplicate domains and invalid weights or opacity bounds.
    fn compose_ensemble(
        &mut self,
        layers: &[EnsembleLayer],
        style: EnsembleCompositionStyle,
    ) -> Result<GenericCompositionView, CompositionError>;
}
