//! Declarative representation presets over the scene.

use super::state::Scene;
use crate::{
    CoreError, RepresentationHandle, RepresentationKind, RepresentationTarget, VolumeHandle,
};

impl Scene {
    /// Adds one declarative preset over a compatible selection or volume.
    ///
    /// Signed isosurfaces return two handles in negative-then-positive order;
    /// every other preset returns one.
    ///
    /// # Errors
    ///
    /// Returns a typed error for stale targets, mismatched target families or
    /// malformed signed levels.
    pub fn represent_preset(
        &mut self,
        target: RepresentationTarget,
        preset: crate::RepresentationPreset,
    ) -> Result<Vec<RepresentationHandle>, CoreError> {
        match (target, preset) {
            (RepresentationTarget::Selection(selection), crate::RepresentationPreset::Cpk) => {
                let handle = self.represent(selection, RepresentationKind::BallAndStick)?;
                if let Some(representation) = self.representation_mut(handle) {
                    representation.params.radius_scale = 0.7;
                    representation.params.bond_radius = 0.3;
                }
                Ok(vec![handle])
            }
            (RepresentationTarget::Selection(selection), crate::RepresentationPreset::Licorice) => {
                Ok(vec![
                    self.represent(selection, RepresentationKind::Licorice)?,
                ])
            }
            (
                RepresentationTarget::Selection(selection),
                crate::RepresentationPreset::PaperChain,
            ) => Ok(vec![
                self.represent(selection, RepresentationKind::PaperChain)?,
            ]),
            (
                RepresentationTarget::Selection(selection),
                crate::RepresentationPreset::DottedSolvent,
            ) => {
                let handle = self.represent(selection, RepresentationKind::Surface)?;
                if let Some(representation) = self.representation_mut(handle) {
                    representation.params.surface_kind = crate::SurfaceKind::SolventAccessible;
                    representation.params.surface_style = crate::SurfaceStyle::Dots;
                }
                Ok(vec![handle])
            }
            (
                RepresentationTarget::Volume(volume),
                crate::RepresentationPreset::SignedIsosurface {
                    negative_level,
                    positive_level,
                    negative_color,
                    positive_color,
                },
            ) => self.represent_signed_isosurfaces(
                volume,
                negative_level,
                positive_level,
                negative_color,
                positive_color,
            ),
            _ => Err(CoreError::InvalidSelection {
                reason: "representation preset is incompatible with its target",
            }),
        }
    }

    fn represent_signed_isosurfaces(
        &mut self,
        volume: VolumeHandle,
        negative_level: f32,
        positive_level: f32,
        negative_color: molgfx_math::Rgba8,
        positive_color: molgfx_math::Rgba8,
    ) -> Result<Vec<RepresentationHandle>, CoreError> {
        if !negative_level.is_finite()
            || !positive_level.is_finite()
            || negative_level >= 0.0
            || positive_level <= 0.0
        {
            return Err(CoreError::InvalidVolume {
                reason: "signed isosurfaces require one negative and one positive level",
            });
        }
        let recipe = |level, color| {
            crate::Representation::volume()
                .isolevel(level)
                .volume_style(crate::VolumeStyle::isosurface().transfer(
                    crate::VolumeTransferFunction::linear(
                        [negative_level, positive_level],
                        color,
                        color,
                    ),
                ))
        };
        let negative = self.represent(volume, recipe(negative_level, negative_color))?;
        let positive = self.represent(volume, recipe(positive_level, positive_color))?;
        Ok(vec![negative, positive])
    }
}
