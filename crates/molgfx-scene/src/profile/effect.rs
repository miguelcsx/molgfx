//! Explicit presentation modules applied over the quality recipe.

use super::DepthCue;
use crate::Error;

pub use molgfx_render::{
    BackdropStyle, BloomStyle, DepthOfField, DisplayTransform, FocusTarget, LightingEnvironment,
    MotionBlur, ShapeCueStyle,
};

/// A post-resolve anti-aliasing choice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AntiAliasing {
    /// Do not smooth edges after temporal resolution.
    Off,
    /// Use the existing edge-aware tonemap filter.
    Fxaa,
}

/// Identity of one presentation module.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EffectKind {
    /// View-space depth fade.
    DepthCue,
    /// Image edge smoothing.
    AntiAliasing,
    /// Highlight bleed.
    Bloom,
    /// Thin-lens defocus.
    DepthOfField,
    /// Camera-shutter blur.
    MotionBlur,
    /// Compositing fallback.
    Backdrop,
    /// Scene illumination.
    Lighting,
    /// Molecular shape cues.
    ShapeCues,
    /// Display grading.
    Display,
}

/// One explicit presentation override. Renderer-owned payloads are reused so
/// the renderer and the facade cannot disagree about field meaning.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Effect {
    /// View-space depth fade.
    DepthCue(DepthCue),
    /// Image edge smoothing.
    AntiAliasing(AntiAliasing),
    /// Highlight bleed.
    Bloom(BloomStyle),
    /// Thin-lens defocus.
    DepthOfField(DepthOfField),
    /// Camera-shutter blur.
    MotionBlur(MotionBlur),
    /// Compositing fallback.
    Backdrop(BackdropStyle),
    /// Scene illumination.
    Lighting(LightingEnvironment),
    /// Molecular shape cues.
    ShapeCues(ShapeCueStyle),
    /// Display grading.
    Display(DisplayTransform),
}

impl Effect {
    /// The slot replaced by this effect.
    #[must_use]
    pub const fn kind(self) -> EffectKind {
        match self {
            Self::DepthCue(_) => EffectKind::DepthCue,
            Self::AntiAliasing(_) => EffectKind::AntiAliasing,
            Self::Bloom(_) => EffectKind::Bloom,
            Self::DepthOfField(_) => EffectKind::DepthOfField,
            Self::MotionBlur(_) => EffectKind::MotionBlur,
            Self::Backdrop(_) => EffectKind::Backdrop,
            Self::Lighting(_) => EffectKind::Lighting,
            Self::ShapeCues(_) => EffectKind::ShapeCues,
            Self::Display(_) => EffectKind::Display,
        }
    }

    pub(crate) fn render(self) -> molgfx_render::PresentationEffect {
        use molgfx_render::PresentationEffect as R;
        match self {
            Self::DepthCue(cue) => R::DepthCue(molgfx_render::DepthCue {
                near_distance: cue.near_distance(),
                far_distance: cue.far_distance(),
                strength: cue.strength(),
            }),
            Self::AntiAliasing(choice) => R::AntiAliasing(molgfx_render::AntiAliasingStyle {
                edge_smoothing: choice == AntiAliasing::Fxaa,
            }),
            Self::Bloom(value) => R::Bloom(value),
            Self::DepthOfField(value) => R::DepthOfField(value),
            Self::MotionBlur(value) => R::MotionBlur(value),
            Self::Backdrop(value) => R::Backdrop(value),
            Self::Lighting(value) => R::Lighting(value),
            Self::ShapeCues(value) => R::ShapeCues(value),
            Self::Display(value) => R::Display(value),
        }
    }

    /// Validates numeric payloads before renderer construction rather than
    /// relying on the lower-level render plan's defensive sanitization.
    ///
    /// # Errors
    /// Returns `InvalidSpec` for out-of-range or non-finite fields.
    pub fn validate(self) -> Result<Self, Error> {
        let valid = match self {
            Self::DepthCue(_) | Self::AntiAliasing(_) => true,
            Self::Bloom(v) => {
                in_range(v.threshold, 0.0, 64.0)
                    && in_range(v.intensity, 0.0, 1.0)
                    && in_range(v.radius, 0.0, 8.0)
            }
            Self::DepthOfField(v) => {
                in_range(v.focal_length_mm, 1.0, 300.0)
                    && in_range(v.f_number, 0.7, 64.0)
                    && in_range(v.sensor_width_mm, 1.0, 100.0)
                    && in_range(v.max_blur_pixels, 0.0, 64.0)
                    && (3..=12).contains(&v.blade_count)
                    && match v.focus {
                        FocusTarget::CameraTarget | FocusTarget::Selection(_) => true,
                        FocusTarget::Distance(d) => d.is_finite() && d > 0.0,
                        FocusTarget::WorldPoint(p) => p.is_finite(),
                    }
            }
            Self::MotionBlur(v) => {
                in_range(v.shutter, 0.0, 1.0) && in_range(v.max_blur_pixels, 0.0, 96.0)
            }
            Self::Backdrop(v) => in_range(v.glow_strength, 0.0, 1.0),
            Self::Lighting(v) => {
                valid_direction(v.key_direction)
                    && valid_direction(v.fill_direction)
                    && in_range(v.diffuse_strength, 0.0, 4.0)
                    && in_range(v.specular_strength, 0.0, 4.0)
                    && in_range(v.rim_strength, 0.0, 2.0)
                    && in_range(v.key_strength, 0.0, 8.0)
                    && in_range(v.fill_strength, 0.0, 8.0)
                    && in_range(v.key_angular_radius, 0.0, 0.5)
                    && in_range(v.shadow_strength, 0.0, 1.0)
            }
            Self::ShapeCues(v) => {
                in_range(v.silhouette_strength, 0.0, 1.0)
                    && in_range(v.cavity_strength, 0.0, 1.0)
                    && in_range(v.depth_cue_strength, 0.0, 1.0)
                    && (v.posterize_levels == 0.0 || in_range(v.posterize_levels, 2.0, 16.0))
                    && in_range(v.motion_persistence, 0.0, 1.0)
                    && in_range(v.outline_width, 0.0, 8.0)
            }
            Self::Display(v) => {
                in_range(v.exposure_ev, -8.0, 8.0)
                    && in_range(v.contrast, 0.5, 1.5)
                    && in_range(v.saturation, 0.0, 1.5)
                    && in_range(v.vignette_strength, 0.0, 1.0)
                    && in_range(v.peak_luminance_nits, 80.0, 10_000.0)
            }
        };
        if valid {
            Ok(self)
        } else {
            Err(Error::InvalidSpec(format!(
                "invalid {:?} effect settings",
                self.kind()
            )))
        }
    }
}

fn valid_direction(direction: molgfx_math::Vec3) -> bool {
    let length_squared = direction.length_squared();
    direction.is_finite() && length_squared.is_finite() && length_squared > 0.0
}

fn in_range(value: f32, min: f32, max: f32) -> bool {
    value.is_finite() && (min..=max).contains(&value)
}

/// One optional override per effect kind; resolution follows a stable order.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct EffectSet {
    depth_cue: Option<DepthCue>,
    antialias: Option<AntiAliasing>,
    bloom: Option<BloomStyle>,
    depth_of_field: Option<DepthOfField>,
    motion_blur: Option<MotionBlur>,
    backdrop: Option<BackdropStyle>,
    lighting: Option<LightingEnvironment>,
    shape_cues: Option<ShapeCueStyle>,
    display: Option<DisplayTransform>,
}

impl EffectSet {
    /// No explicit presentation overrides.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            depth_cue: None,
            antialias: None,
            bloom: None,
            depth_of_field: None,
            motion_blur: None,
            backdrop: None,
            lighting: None,
            shape_cues: None,
            display: None,
        }
    }
    /// Finds an explicitly configured effect, excluding quality defaults.
    #[must_use]
    pub const fn effect(self, kind: EffectKind) -> Option<Effect> {
        match kind {
            EffectKind::DepthCue => match self.depth_cue {
                Some(v) => Some(Effect::DepthCue(v)),
                None => None,
            },
            EffectKind::AntiAliasing => match self.antialias {
                Some(v) => Some(Effect::AntiAliasing(v)),
                None => None,
            },
            EffectKind::Bloom => match self.bloom {
                Some(v) => Some(Effect::Bloom(v)),
                None => None,
            },
            EffectKind::DepthOfField => match self.depth_of_field {
                Some(v) => Some(Effect::DepthOfField(v)),
                None => None,
            },
            EffectKind::MotionBlur => match self.motion_blur {
                Some(v) => Some(Effect::MotionBlur(v)),
                None => None,
            },
            EffectKind::Backdrop => match self.backdrop {
                Some(v) => Some(Effect::Backdrop(v)),
                None => None,
            },
            EffectKind::Lighting => match self.lighting {
                Some(v) => Some(Effect::Lighting(v)),
                None => None,
            },
            EffectKind::ShapeCues => match self.shape_cues {
                Some(v) => Some(Effect::ShapeCues(v)),
                None => None,
            },
            EffectKind::Display => match self.display {
                Some(v) => Some(Effect::Display(v)),
                None => None,
            },
        }
    }

    pub(crate) fn with(mut self, effect: Effect) -> Self {
        match effect {
            Effect::DepthCue(v) => self.depth_cue = Some(v),
            Effect::AntiAliasing(v) => self.antialias = Some(v),
            Effect::Bloom(v) => self.bloom = Some(v),
            Effect::DepthOfField(v) => self.depth_of_field = Some(v),
            Effect::MotionBlur(v) => self.motion_blur = Some(v),
            Effect::Backdrop(v) => self.backdrop = Some(v),
            Effect::Lighting(v) => self.lighting = Some(v),
            Effect::ShapeCues(v) => self.shape_cues = Some(v),
            Effect::Display(v) => self.display = Some(v),
        }
        self
    }

    pub(crate) fn without(mut self, kind: EffectKind) -> Self {
        match kind {
            EffectKind::DepthCue => self.depth_cue = None,
            EffectKind::AntiAliasing => self.antialias = None,
            EffectKind::Bloom => self.bloom = None,
            EffectKind::DepthOfField => self.depth_of_field = None,
            EffectKind::MotionBlur => self.motion_blur = None,
            EffectKind::Backdrop => self.backdrop = None,
            EffectKind::Lighting => self.lighting = None,
            EffectKind::ShapeCues => self.shape_cues = None,
            EffectKind::Display => self.display = None,
        }
        self
    }

    pub(crate) fn apply(
        self,
        mut profile: molgfx_render::RenderProfile,
    ) -> molgfx_render::RenderProfile {
        const KINDS: [EffectKind; 9] = [
            EffectKind::DepthCue,
            EffectKind::AntiAliasing,
            EffectKind::Bloom,
            EffectKind::DepthOfField,
            EffectKind::MotionBlur,
            EffectKind::Backdrop,
            EffectKind::Lighting,
            EffectKind::ShapeCues,
            EffectKind::Display,
        ];
        for kind in KINDS {
            if let Some(effect) = self.effect(kind) {
                profile = profile.with_effect(effect.render());
            }
        }
        profile
    }
}
