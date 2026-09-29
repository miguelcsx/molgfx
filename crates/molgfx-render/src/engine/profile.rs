//! Reusable presentation recipes and their allocation-free frame state.
//!
//! Profiles are resolved only when construction settings change. Rendering
//! reads the compact resolved plan, so profile composition adds no per-frame
//! allocation or dynamic dispatch.

pub(super) use super::optics::{DepthOfField, MotionBlur};
use super::profile_numeric::{finite_clamp, lerp, sanitize_bands, unit};
use super::{BackdropStyle, DepthCue, DisplayTransform, LightingEnvironment};
use serde::{Deserialize, Serialize};

#[path = "profile_resolve.rs"]
mod resolve;

/// Screen-space cues that clarify molecular shape without changing geometry
/// or physical colour mappings.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct IllustrationStyle {
    /// Darkening at relative depth and normal discontinuities, in `[0, 1]`.
    pub silhouette_strength: f32,
    /// Bounded emphasis of locally concave depth, in `[0, 1]`.
    pub cavity_strength: f32,
    /// Distance-based fade toward the background, in `[0, 1]`.
    pub depth_cue_strength: f32,
    /// Cel-shading tone bands (clamped `[2, 16]`); zero keeps continuous shading.
    #[serde(default)]
    pub posterize_levels: f32,
    /// Motion-trail persistence `[0, 1]`; zero keeps crisp TAA, higher values
    /// retain a bounded exponentially decaying screen-space history.
    #[serde(default)]
    pub motion_persistence: f32,
    /// Silhouette outline thickness in pixels (clamped `[0, 8]`); zero is a 1px edge.
    #[serde(default)]
    pub outline_width: f32,
}

/// Bright-pass highlight bleed for cinematic presentation.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct BloomStyle {
    /// Scene-linear luminance above which light begins to bleed.
    pub threshold: f32,
    /// Contribution added back over the resolved frame, in `[0, 1]`.
    pub intensity: f32,
    /// Blur reach in quarter-resolution texels, in `[0, 8]`.
    pub radius: f32,
}

impl BloomStyle {
    /// A restrained lens bleed that only the brightest speculars trigger.
    #[must_use]
    pub const fn cinematic() -> Self {
        Self {
            threshold: 1.7,
            intensity: 0.26,
            radius: 2.0,
        }
    }

    fn sanitize(self) -> Self {
        Self {
            threshold: finite_clamp(self.threshold, 0.0, 64.0, 1.05),
            intensity: unit(self.intensity),
            radius: finite_clamp(self.radius, 0.0, 8.0, 2.0),
        }
    }

    fn blend(self, other: Self, weight: f32) -> Self {
        let weight = unit(weight);
        Self {
            threshold: lerp(self.threshold, other.threshold, weight),
            intensity: lerp(self.intensity, other.intensity, weight),
            radius: lerp(self.radius, other.radius, weight),
        }
        .sanitize()
    }

    pub(crate) fn packed(self) -> [f32; 4] {
        let style = self.sanitize();
        [style.threshold, style.intensity, style.radius, 0.0]
    }
}

impl IllustrationStyle {
    /// A restrained publication-style treatment.
    #[must_use]
    pub const fn publication() -> Self {
        Self {
            silhouette_strength: 0.65,
            cavity_strength: 0.35,
            depth_cue_strength: 0.15,
            posterize_levels: 0.0,
            motion_persistence: 0.0,
            outline_width: 0.0,
        }
    }

    fn sanitize(self) -> Self {
        Self {
            silhouette_strength: unit(self.silhouette_strength),
            cavity_strength: unit(self.cavity_strength),
            depth_cue_strength: unit(self.depth_cue_strength),
            posterize_levels: sanitize_bands(self.posterize_levels),
            motion_persistence: unit(self.motion_persistence),
            outline_width: finite_clamp(self.outline_width, 0.0, 8.0, 0.0),
        }
    }

    fn blend(self, other: Self, weight: f32) -> Self {
        let weight = unit(weight);
        Self {
            silhouette_strength: lerp(self.silhouette_strength, other.silhouette_strength, weight),
            cavity_strength: lerp(self.cavity_strength, other.cavity_strength, weight),
            depth_cue_strength: lerp(self.depth_cue_strength, other.depth_cue_strength, weight),
            posterize_levels: lerp(self.posterize_levels, other.posterize_levels, weight),
            motion_persistence: lerp(self.motion_persistence, other.motion_persistence, weight),
            outline_width: lerp(self.outline_width, other.outline_width, weight),
        }
    }

    pub(crate) fn packed(self, focus_distance: f32) -> [f32; 4] {
        let style = self.sanitize();
        [
            style.silhouette_strength,
            style.cavity_strength,
            style.depth_cue_strength,
            if focus_distance.is_finite() {
                focus_distance.max(1.0e-3)
            } else {
                1.0
            },
        ]
    }

    /// Non-photorealistic lane: cel band count in `x`, motion-trail
    /// persistence in `y`, outline width in `z`, spare in `w`.
    pub(crate) fn npr_packed(self) -> [f32; 4] {
        let s = self.sanitize();
        [
            s.posterize_levels,
            s.motion_persistence,
            s.outline_width,
            0.0,
        ]
    }
}

/// Anti-aliasing applied to the presented image.
///
/// The engine renders into an HDR history buffer; this setting decides how the
/// resolved image is smoothed on its way to the display. Edge smoothing runs at
/// `O(pixels)` in the tonemap pass and is fused there, so selecting it never
/// adds a pass; the temporal resolve is a separate, always-on mechanism that a
/// moving camera relies on regardless of this choice.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct AntiAliasingStyle {
    /// Whether the tonemap pass smooths edges.
    ///
    /// A still, converged image is already smooth, so the default leaves this
    /// off and lets callers enable it for a realtime or non-converging view.
    pub edge_smoothing: bool,
}

impl AntiAliasingStyle {
    /// Edge smoothing on: the choice for an interactive view that does not
    /// accumulate.
    #[must_use]
    pub const fn smoothed() -> Self {
        Self {
            edge_smoothing: true,
        }
    }

    /// Edge smoothing off: the choice for publication, where the accumulated
    /// image needs no post-filter.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            edge_smoothing: false,
        }
    }

    fn sanitize(self) -> Self {
        self
    }
}

/// A typed presentation module that a render profile can layer.
///
/// The enum is non-exhaustive so new engine effects do not force downstream
/// callers to match every future module.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PresentationEffect {
    /// Molecular illustration applied after opaque lighting.
    Illustration(IllustrationStyle),
    /// Explicit view-space fog/depth cue applied after lighting.
    DepthCue(DepthCue),
    /// Camera-space thin-lens depth of field after temporal resolution.
    DepthOfField(DepthOfField),
    /// Camera-shutter blur from the G-buffer's true surface motion.
    MotionBlur(MotionBlur),
    /// Fallback colour outside represented scene content.
    Backdrop(BackdropStyle),
    /// Reflected, ambient, key and fill illumination, independent of backdrop.
    Lighting(LightingEnvironment),
    /// Exposure and display grading, independent of scene content.
    Display(DisplayTransform),
    /// Bright-pass highlight bleed applied before the display transform.
    Bloom(BloomStyle),
    /// Edge smoothing applied as the image is presented.
    AntiAliasing(AntiAliasingStyle),
}

/// One weighted module in a reusable render profile.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct EffectLayer {
    /// Lower priorities resolve first. Equal priorities preserve insertion
    /// order.
    pub priority: i16,
    /// Interpolation weight, sanitized to `[0, 1]` while resolving.
    pub weight: f32,
    /// Typed effect settings.
    pub effect: PresentationEffect,
}

impl EffectLayer {
    /// Creates a fully weighted layer at priority zero.
    #[must_use]
    pub const fn new(effect: PresentationEffect) -> Self {
        Self {
            priority: 0,
            weight: 1.0,
            effect,
        }
    }

    /// Sets the layer's interpolation weight.
    #[must_use]
    pub const fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }

    /// Sets the layer's ordering priority.
    #[must_use]
    pub const fn with_priority(mut self, priority: i16) -> Self {
        self.priority = priority;
        self
    }
}

/// A reusable, declarative recipe for presentation effects.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct RenderProfile {
    layers: Vec<EffectLayer>,
}

impl RenderProfile {
    /// The quantitative inspection baseline with no optional presentation
    /// modules.
    #[must_use]
    pub const fn inspection() -> Self {
        Self { layers: Vec::new() }
    }

    /// A restrained publication illustration recipe.
    #[must_use]
    pub fn illustrative() -> Self {
        Self::inspection().with_effect(PresentationEffect::Illustration(
            IllustrationStyle::publication(),
        ))
    }

    /// Art-directed molecular-film optics and grading. It deliberately leaves
    /// the fallback backdrop unchanged: biological context must be represented
    /// by structures, solvent, membranes or caller density.
    #[must_use]
    pub fn cinematic() -> Self {
        Self::inspection()
            .with_effect(PresentationEffect::Illustration(IllustrationStyle {
                silhouette_strength: 0.5,
                cavity_strength: 0.4,
                depth_cue_strength: 0.28,
                posterize_levels: 0.0,
                motion_persistence: 0.0,
                outline_width: 0.0,
            }))
            .with_effect(PresentationEffect::Lighting(
                LightingEnvironment::documentary(),
            ))
            .with_effect(PresentationEffect::Display(DisplayTransform::cinematic()))
            .with_effect(PresentationEffect::Bloom(BloomStyle::cinematic()))
            .with_effect(PresentationEffect::DepthOfField(DepthOfField::cinematic()))
            .with_effect(PresentationEffect::MotionBlur(MotionBlur::cinematic()))
    }

    /// Appends a fully weighted module.
    #[must_use]
    pub fn with_effect(self, effect: PresentationEffect) -> Self {
        self.with_layer(EffectLayer::new(effect))
    }

    /// Appends a weighted, prioritized module.
    #[must_use]
    pub fn with_layer(mut self, layer: EffectLayer) -> Self {
        self.layers.push(layer);
        self
    }

    /// Modules in insertion order. Resolution also considers priority.
    #[must_use]
    pub fn layers(&self) -> &[EffectLayer] {
        &self.layers
    }
}

/// The compact, sanitized plan the frame loop actually consumes.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct ResolvedRenderPlan {
    illustration: IllustrationStyle,
    depth_cue: DepthCue,
    depth_of_field: Option<DepthOfField>,
    motion_blur: Option<MotionBlur>,
    bloom: Option<BloomStyle>,
    backdrop: BackdropStyle,
    lighting: LightingEnvironment,
    display: DisplayTransform,
    antialias: Option<AntiAliasingStyle>,
}

impl ResolvedRenderPlan {
    /// The final molecular illustration settings after ordered blending.
    #[must_use]
    pub const fn illustration(&self) -> IllustrationStyle {
        self.illustration
    }
    /// The explicit view-space fog/depth cue, if enabled.
    #[must_use]
    pub const fn depth_cue(&self) -> DepthCue {
        self.depth_cue
    }
    /// The resolved thin-lens module, if the profile contributes blur.
    #[must_use]
    pub const fn depth_of_field(&self) -> Option<DepthOfField> {
        self.depth_of_field
    }

    /// Resolved camera-shutter blur module, if enabled.
    #[must_use]
    pub const fn motion_blur(&self) -> Option<MotionBlur> {
        self.motion_blur
    }

    /// Resolved compositing fallback.
    #[must_use]
    pub const fn backdrop(&self) -> BackdropStyle {
        self.backdrop
    }

    /// Resolved reflected and direct lighting rig.
    #[must_use]
    pub const fn lighting(&self) -> LightingEnvironment {
        self.lighting
    }

    /// Resolved display transform.
    #[must_use]
    pub const fn display(&self) -> DisplayTransform {
        self.display
    }

    /// The resolved highlight bleed, if the profile contributes one.
    #[must_use]
    pub const fn bloom(&self) -> Option<BloomStyle> {
        self.bloom
    }

    /// The resolved edge-smoothing choice, or `None` when no profile layer
    /// stated one and the tier's default applies.
    #[must_use]
    pub const fn antialias(&self) -> Option<AntiAliasingStyle> {
        self.antialias
    }

    pub(crate) fn packed_presentation(self, has_translucency: bool) -> [[f32; 4]; 6] {
        super::backdrop::pack(
            self.backdrop,
            self.display,
            has_translucency,
            match self.bloom {
                Some(style) => style.packed(),
                None => [0.0; 4],
            },
        )
    }

    pub(crate) fn packed_lighting(self) -> [[f32; 4]; 8] {
        self.lighting.packed()
    }

    pub(crate) fn packed_depth_cue(self) -> [f32; 4] {
        self.depth_cue.packed()
    }

    pub(crate) fn optics(self, focus_distance: f32) -> [f32; 4] {
        match self.depth_of_field {
            Some(settings) => settings.packed(focus_distance),
            None => [focus_distance.max(1.0e-3), 0.0, 0.0, 0.0],
        }
    }
}
#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
