//! Biological screen-space-error LOD with hysteresis.

#[cfg(test)]
#[path = "lod_tests.rs"]
mod tests;

/// Persistent biological detail buffers, finest to coarsest.
pub type LodLevel = pdviewx_core::ResidencyDetail;

/// Deterministic projected-error thresholds with transition hysteresis.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LodPolicy {
    /// Atom-to-residue boundary in pixels.
    pub atom_pixels: f32,
    /// Residue-to-secondary-structure boundary in pixels.
    pub residue_pixels: f32,
    /// Secondary-structure-to-domain boundary in pixels.
    pub secondary_pixels: f32,
    /// Fractional dead band around the previous level.
    pub hysteresis: f32,
}

impl Default for LodPolicy {
    fn default() -> Self {
        Self {
            atom_pixels: 4.0,
            residue_pixels: 1.0,
            secondary_pixels: 0.25,
            hysteresis: 0.15,
        }
    }
}

impl LodPolicy {
    /// Selects detail from projected atom error and semantic importance.
    /// Importance is clamped to `[0.25, 4]`, keeping focused clusters detailed.
    #[must_use]
    pub fn select(self, error_pixels: f32, importance: f32, previous: LodLevel) -> LodLevel {
        let weighted = error_pixels.max(0.0) * importance.clamp(0.25, 4.0);
        let candidate = self.unstable_level(weighted);
        if candidate == previous {
            return previous;
        }
        let boundary = self.boundary(previous, candidate);
        let margin = boundary * self.hysteresis.clamp(0.0, 0.49);
        if candidate < previous && weighted <= boundary + margin
            || candidate > previous && weighted >= boundary - margin
        {
            previous
        } else {
            candidate
        }
    }

    fn unstable_level(self, pixels: f32) -> LodLevel {
        if pixels >= self.atom_pixels {
            LodLevel::Atom
        } else if pixels >= self.residue_pixels {
            LodLevel::Residue
        } else if pixels >= self.secondary_pixels {
            LodLevel::SecondaryStructure
        } else {
            LodLevel::Domain
        }
    }

    fn boundary(self, a: LodLevel, b: LodLevel) -> f32 {
        match a.min(b) {
            LodLevel::Atom => self.atom_pixels,
            LodLevel::Residue => self.residue_pixels,
            LodLevel::SecondaryStructure | LodLevel::Domain => self.secondary_pixels,
        }
    }
}
