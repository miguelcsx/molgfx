//! Reusable presentation recipes and their allocation-free frame state.
//!
//! Profiles are resolved only when construction settings change. Rendering
//! reads the compact resolved plan, so profile composition adds no per-frame
//! allocation or dynamic dispatch.

use super::profile_numeric::{finite_clamp, lerp, sanitize_bands, unit};
use super::{BackdropStyle, DisplayTransform, LightingEnvironment};
use molgfx_core::SelectionHandle;
use molgfx_math::Vec3;
use serde::{Deserialize, Serialize};

#[path = "profile_resolve.rs"]
mod resolve;

/// A physical or scene-tracked focal plane for thin-lens presentation.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub enum FocusTarget {
    /// Follow the camera's look-at target.
    #[default]
    CameraTarget,
    /// Use an explicit positive view-space distance in Ångström.
    Distance(f32),
    /// Track one world-space point as the camera moves.
    WorldPoint(Vec3),
    /// Track the centroid of a caller-authored molecular selection.
    Selection(SelectionHandle),
}

impl FocusTarget {
    fn sanitize(self) -> Self {
        match self {
            Self::Distance(distance) if distance.is_finite() && distance > 0.0 => self,
            Self::WorldPoint(point) if point.is_finite() => self,
            Self::Selection(_) | Self::CameraTarget => self,
            Self::Distance(_) | Self::WorldPoint(_) => Self::CameraTarget,
        }
    }
}

/// Screen-space cues that clarify molecular shape without changing geometry
/// or scientific colour mappings.
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

/// Thin-lens depth-of-field settings for cinematic presentation.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct DepthOfField {
    /// Lens focal length, millimetres.
    pub focal_length_mm: f32,
    /// Aperture f-number.
    pub f_number: f32,
    /// Horizontal sensor extent, millimetres.
    pub sensor_width_mm: f32,
    /// Maximum circle-of-confusion radius, physical pixels.
    pub max_blur_pixels: f32,
    /// Aperture blade count, clamped to `[3, 12]`.
    pub blade_count: u8,
    /// Scene target from which the focal plane is resolved.
    pub focus: FocusTarget,
}

impl DepthOfField {
    /// A restrained full-frame macro-lens recipe for molecular cinematics.
    #[must_use]
    pub const fn cinematic() -> Self {
        Self {
            focal_length_mm: 50.0,
            // Stopped well down on purpose. A molecule is a deep subject: at a
            // wide aperture its own front and back fall outside the focal
            // range and the whole specimen goes soft, which reads as a
            // low-resolution image rather than a photographic one. This keeps
            // the subject crisp and spends the blur on what is genuinely far
            // from the focal plane.
            f_number: 8.0,
            sensor_width_mm: 36.0,
            max_blur_pixels: 14.0,
            blade_count: 7,
            focus: FocusTarget::CameraTarget,
        }
    }

    fn sanitize(self) -> Self {
        Self {
            focal_length_mm: finite_clamp(self.focal_length_mm, 1.0, 300.0, 50.0),
            f_number: finite_clamp(self.f_number, 0.7, 64.0, 4.0),
            sensor_width_mm: finite_clamp(self.sensor_width_mm, 1.0, 100.0, 36.0),
            max_blur_pixels: finite_clamp(self.max_blur_pixels, 0.0, 64.0, 0.0),
            blade_count: self.blade_count.clamp(3, 12),
            focus: self.focus.sanitize(),
        }
    }

    fn blend(self, other: Self, weight: f32) -> Self {
        let weight = unit(weight);
        Self {
            focal_length_mm: lerp(self.focal_length_mm, other.focal_length_mm, weight),
            f_number: lerp(self.f_number, other.f_number, weight),
            sensor_width_mm: lerp(self.sensor_width_mm, other.sensor_width_mm, weight),
            max_blur_pixels: lerp(self.max_blur_pixels, other.max_blur_pixels, weight),
            blade_count: if weight >= 0.5 {
                other.blade_count
            } else {
                self.blade_count
            },
            focus: if weight >= 0.5 {
                other.focus
            } else {
                self.focus
            },
        }
        .sanitize()
    }

    pub(crate) fn packed(self, focus_distance: f32) -> [f32; 4] {
        let settings = self.sanitize();
        [
            focus_distance.max(1.0e-3),
            settings.focal_length_mm / (settings.f_number * settings.sensor_width_mm),
            settings.max_blur_pixels,
            f32::from(settings.blade_count),
        ]
    }
}

/// Camera-shutter motion blur driven by the renderer's true object motion.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct MotionBlur {
    /// Fraction of the frame interval exposed by the virtual shutter.
    pub shutter: f32,
    /// Maximum blur length in physical pixels.
    pub max_blur_pixels: f32,
}

impl MotionBlur {
    /// A restrained cinematic shutter that preserves molecular legibility.
    #[must_use]
    pub const fn cinematic() -> Self {
        Self {
            shutter: 0.55,
            max_blur_pixels: 18.0,
        }
    }

    fn sanitize(self) -> Self {
        Self {
            shutter: unit(self.shutter),
            max_blur_pixels: finite_clamp(self.max_blur_pixels, 0.0, 96.0, 18.0),
        }
    }

    fn blend(self, other: Self, weight: f32) -> Self {
        let weight = unit(weight);
        Self {
            shutter: lerp(self.shutter, other.shutter, weight),
            max_blur_pixels: lerp(self.max_blur_pixels, other.max_blur_pixels, weight),
        }
        .sanitize()
    }

    pub(crate) fn packed(self) -> [f32; 4] {
        let value = self.sanitize();
        [value.shutter, value.max_blur_pixels, 0.0, 0.0]
    }
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

/// A typed presentation module that a render profile can layer.
///
/// The enum is non-exhaustive so new engine effects do not force downstream
/// callers to match every future module.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PresentationEffect {
    /// Molecular illustration applied after opaque lighting.
    Illustration(IllustrationStyle),
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
    depth_of_field: Option<DepthOfField>,
    motion_blur: Option<MotionBlur>,
    bloom: Option<BloomStyle>,
    backdrop: BackdropStyle,
    lighting: LightingEnvironment,
    display: DisplayTransform,
}

impl ResolvedRenderPlan {
    /// The final molecular illustration settings after ordered blending.
    #[must_use]
    pub const fn illustration(&self) -> IllustrationStyle {
        self.illustration
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
