//! Compositing fallback and display transform.
//!
//! Neither type represents biology. Biological surroundings enter the scene as
//! structures, declared solvent, membranes or caller-provided density fields.

use pdviewx_math::Rgba8;
use serde::{Deserialize, Serialize};

/// Colour shown only where no represented biology contributes a fragment.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct BackdropStyle {
    /// Upper fallback colour in display sRGB.
    pub top: Rgba8,
    /// Lower fallback colour in display sRGB.
    pub bottom: Rgba8,
    /// Optional central compositing glow in display sRGB.
    pub glow_color: Rgba8,
    /// Glow contribution in `[0, 1]`.
    pub glow_strength: f32,
}

impl BackdropStyle {
    /// Neutral publication/compositing fallback.
    #[must_use]
    pub const fn compositing() -> Self {
        Self {
            top: Rgba8::opaque(244, 247, 250),
            bottom: Rgba8::opaque(218, 224, 232),
            glow_color: Rgba8::opaque(255, 255, 255),
            glow_strength: 0.08,
        }
    }

    /// Transparent compositing fallback retaining neutral RGB fringe colours.
    ///
    /// Opaque molecular fragments remain opaque and translucent scene matter
    /// contributes its accumulated coverage to the exported alpha channel.
    #[must_use]
    pub const fn transparent() -> Self {
        Self {
            top: Rgba8::new(244, 247, 250, 0),
            bottom: Rgba8::new(218, 224, 232, 0),
            glow_color: Rgba8::new(255, 255, 255, 0),
            glow_strength: 0.0,
        }
    }

    /// A bright, near-neutral studio sweep with a soft central pool of light.
    ///
    /// A specimen photographed in a lightbox reads as a real object: the eye
    /// has a lit ground to measure the subject against, and shadow and
    /// occlusion — which carry the shape — stay legible because they are darker
    /// than their surroundings rather than lost in an already-black frame. The
    /// faint cool cast is a lighting choice, not depicted matter.
    #[must_use]
    pub const fn studio() -> Self {
        Self {
            top: Rgba8::opaque(238, 241, 245),
            bottom: Rgba8::opaque(206, 214, 223),
            glow_color: Rgba8::opaque(255, 255, 255),
            glow_strength: 0.30,
        }
    }

    pub(crate) fn sanitize(self) -> Self {
        Self {
            glow_strength: unit(self.glow_strength),
            ..self
        }
    }

    pub(crate) fn blend(self, other: Self, weight: f32) -> Self {
        let weight = unit(weight);
        if weight <= 0.0 {
            return self.sanitize();
        }
        if weight >= 1.0 {
            return other.sanitize();
        }
        Self {
            top: choose_color(self.top, other.top, weight),
            bottom: choose_color(self.bottom, other.bottom, weight),
            glow_color: choose_color(self.glow_color, other.glow_color, weight),
            glow_strength: lerp(self.glow_strength, other.glow_strength, weight),
        }
        .sanitize()
    }
}

impl Default for BackdropStyle {
    fn default() -> Self {
        Self::studio()
    }
}

/// Scene-linear exposure and display grading, independent of the backdrop.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ToneMapping {
    /// ACES fitted SDR curve used by the publication and cinematic defaults.
    AcesFitted,
    /// Simple scene-linear Reinhard compression for diagnostic output.
    Reinhard,
    /// Unmapped scene-linear output for callers that own the display transform.
    None,
}

impl ToneMapping {
    pub(crate) const fn tag(self) -> f32 {
        match self {
            Self::AcesFitted => 0.0,
            Self::Reinhard => 1.0,
            Self::None => 2.0,
        }
    }
}

/// Display primaries used after the scene-linear presentation transform.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum DisplayGamut {
    /// Rec.709 primaries used by sRGB displays.
    #[default]
    Srgb,
    /// DCI-P3 primaries with a D65 white point.
    DisplayP3,
    /// Wide-gamut Rec.2020 primaries.
    Rec2020,
}

impl DisplayGamut {
    /// Every gamut, in pipeline-variant order.
    pub(crate) const ALL: [Self; 3] = [Self::Srgb, Self::DisplayP3, Self::Rec2020];

    /// Index of this gamut's pre-built pipeline variant.
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Srgb => 0,
            Self::DisplayP3 => 1,
            Self::Rec2020 => 2,
        }
    }

    pub(crate) const fn tag(self) -> f32 {
        match self {
            Self::Srgb => 0.0,
            Self::DisplayP3 => 1.0,
            Self::Rec2020 => 2.0,
        }
    }
}

/// Transfer curve used to encode the selected display gamut.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum TransferFunction {
    /// Standard dynamic-range sRGB electro-optical transfer curve.
    #[default]
    Srgb,
    /// Linear values for callers that own the final encoding.
    Linear,
    /// SMPTE ST 2084 perceptual quantizer for HDR output.
    Pq,
    /// Hybrid Log-Gamma scene-referred HDR transfer curve.
    Hlg,
}

impl TransferFunction {
    /// Every transfer curve, in pipeline-variant order.
    pub(crate) const ALL: [Self; 4] = [Self::Srgb, Self::Linear, Self::Pq, Self::Hlg];

    /// Index of this curve's pre-built pipeline variant.
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Srgb => 0,
            Self::Linear => 1,
            Self::Pq => 2,
            Self::Hlg => 3,
        }
    }

    pub(crate) const fn tag(self) -> f32 {
        match self {
            Self::Srgb => 0.0,
            Self::Linear => 1.0,
            Self::Pq => 2.0,
            Self::Hlg => 3.0,
        }
    }
}

/// Scene-linear exposure and display grading, independent of the backdrop.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct DisplayTransform {
    /// Exposure compensation in stops.
    pub exposure_ev: f32,
    /// Display contrast multiplier in `[0.5, 1.5]`.
    pub contrast: f32,
    /// Display saturation multiplier in `[0, 1.5]`.
    pub saturation: f32,
    /// Optical edge falloff in `[0, 1]`.
    pub vignette_strength: f32,
    /// Explicit output tone operator.
    pub tone_mapping: ToneMapping,
    /// Output display primaries.
    #[serde(default = "default_display_gamut")]
    pub gamut: DisplayGamut,
    /// Output electro-optical transfer curve.
    #[serde(default = "default_transfer_function")]
    pub transfer: TransferFunction,
    /// Luminance represented by a unit scene-linear output for PQ, in nits.
    #[serde(default = "default_peak_luminance")]
    pub peak_luminance_nits: f32,
}

impl DisplayTransform {
    /// Restrained art-directed grade without selecting a backdrop.
    #[must_use]
    pub const fn cinematic() -> Self {
        Self {
            exposure_ev: 0.22,
            contrast: 1.18,
            saturation: 1.3,
            vignette_strength: 0.16,
            tone_mapping: ToneMapping::AcesFitted,
            gamut: DisplayGamut::Srgb,
            transfer: TransferFunction::Srgb,
            peak_luminance_nits: 100.0,
        }
    }

    pub(crate) fn sanitize(self) -> Self {
        Self {
            exposure_ev: finite_clamp(self.exposure_ev, -8.0, 8.0, 0.0),
            contrast: finite_clamp(self.contrast, 0.5, 1.5, 1.0),
            saturation: finite_clamp(self.saturation, 0.0, 1.5, 1.0),
            vignette_strength: unit(self.vignette_strength),
            tone_mapping: self.tone_mapping,
            gamut: self.gamut,
            transfer: self.transfer,
            peak_luminance_nits: finite_clamp(self.peak_luminance_nits, 80.0, 10_000.0, 100.0),
        }
    }

    pub(crate) fn blend(self, other: Self, weight: f32) -> Self {
        let weight = unit(weight);
        Self {
            exposure_ev: lerp(self.exposure_ev, other.exposure_ev, weight),
            contrast: lerp(self.contrast, other.contrast, weight),
            saturation: lerp(self.saturation, other.saturation, weight),
            vignette_strength: lerp(self.vignette_strength, other.vignette_strength, weight),
            tone_mapping: if weight >= 0.5 {
                other.tone_mapping
            } else {
                self.tone_mapping
            },
            gamut: if weight >= 0.5 {
                other.gamut
            } else {
                self.gamut
            },
            transfer: if weight >= 0.5 {
                other.transfer
            } else {
                self.transfer
            },
            peak_luminance_nits: lerp(self.peak_luminance_nits, other.peak_luminance_nits, weight),
        }
        .sanitize()
    }
}

impl Default for DisplayTransform {
    fn default() -> Self {
        Self {
            exposure_ev: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            vignette_strength: 0.0,
            tone_mapping: ToneMapping::AcesFitted,
            gamut: DisplayGamut::Srgb,
            transfer: TransferFunction::Srgb,
            peak_luminance_nits: 100.0,
        }
    }
}

pub(crate) fn pack(
    backdrop: BackdropStyle,
    display: DisplayTransform,
    has_translucency: bool,
    bloom: [f32; 4],
) -> [[f32; 4]; 6] {
    let backdrop = backdrop.sanitize();
    let display = display.sanitize();
    let top = linear_rgb(backdrop.top);
    let bottom = linear_rgb(backdrop.bottom);
    let glow = linear_rgb(backdrop.glow_color);
    let display_encoding = display.gamut.tag() + display.transfer.tag() * 4.0;
    [
        [top[0], top[1], top[2], backdrop.glow_strength],
        [
            bottom[0],
            bottom[1],
            bottom[2],
            2.0_f32.powf(display.exposure_ev),
        ],
        [glow[0], glow[1], glow[2], display.contrast],
        [
            display.saturation,
            display.vignette_strength,
            display.tone_mapping.tag(),
            0.42,
        ],
        [
            f32::from(backdrop.top.a) / 255.0,
            f32::from(backdrop.bottom.a) / 255.0,
            if has_translucency { 1.0 } else { 0.0 },
            display_encoding,
        ],
        [bloom[0], bloom[1], bloom[2], display.peak_luminance_nits],
    ]
}

const fn default_display_gamut() -> DisplayGamut {
    DisplayGamut::Srgb
}

const fn default_transfer_function() -> TransferFunction {
    TransferFunction::Srgb
}

const fn default_peak_luminance() -> f32 {
    100.0
}

fn choose_color(current: Rgba8, next: Rgba8, weight: f32) -> Rgba8 {
    if weight >= 0.5 { next } else { current }
}

pub(crate) fn linear_rgb(color: Rgba8) -> [f32; 3] {
    let normalized = color.to_f32();
    [
        srgb_to_linear(normalized[0]),
        srgb_to_linear(normalized[1]),
        srgb_to_linear(normalized[2]),
    ]
}

fn srgb_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn lerp(from: f32, to: f32, weight: f32) -> f32 {
    from + (to - from) * weight
}

fn finite_clamp(value: f32, minimum: f32, maximum: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(minimum, maximum)
    } else {
        fallback
    }
}
