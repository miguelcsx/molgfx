//! Reusable weighted-ensemble presentation over ordinary scene primitives.

use pdviewx_core::{
    AtomSelection, CoreError, EnsembleHandle, Material, RepresentationHandle, RepresentationKind,
    Scene, SelectionHandle, VolumeHandle,
};
use pdviewx_math::Rgba8;

#[cfg(test)]
#[path = "ensemble_tests.rs"]
mod tests;

const MEMBER_COLORS: [Rgba8; 8] = [
    Rgba8::opaque(68, 119, 170),
    Rgba8::opaque(238, 102, 119),
    Rgba8::opaque(34, 136, 51),
    Rgba8::opaque(204, 187, 68),
    Rgba8::opaque(102, 204, 238),
    Rgba8::opaque(170, 51, 119),
    Rgba8::opaque(238, 153, 0),
    Rgba8::opaque(187, 187, 187),
];

/// Declarative weighted-overlay presentation.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EnsembleStyle {
    /// Geometry shared by every member.
    pub representation: RepresentationKind,
    /// Maximum opacity of non-dominant members.
    pub alternate_opacity: f32,
    /// Lower bound keeping low-population members discoverable.
    pub minimum_opacity: f32,
}

impl Default for EnsembleStyle {
    fn default() -> Self {
        Self {
            representation: RepresentationKind::Cartoon,
            alternate_opacity: 0.55,
            minimum_opacity: 0.08,
        }
    }
}

/// Editable representation set produced for an ensemble overlay.
#[derive(Clone, PartialEq, Debug)]
pub struct EnsembleView {
    /// Source weighted ensemble.
    pub ensemble: EnsembleHandle,
    /// One structure-scoped selection per member.
    pub selections: Vec<SelectionHandle>,
    /// One independently editable representation per member.
    pub representations: Vec<RepresentationHandle>,
    /// Dominant member index, solid in overlay mode.
    pub dominant: usize,
}

impl EnsembleView {
    /// Switches from overlay to one visible member for cycle/playback UIs.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidEnsemble`] for an out-of-range member.
    pub fn show_member(&self, scene: &mut Scene, member: usize) -> Result<(), CoreError> {
        if member >= self.representations.len() {
            return Err(CoreError::InvalidEnsemble {
                reason: "cycle member index is outside the ensemble",
            });
        }
        for (index, &representation) in self.representations.iter().enumerate() {
            if index == member {
                scene.show(representation);
            } else {
                scene.hide(representation);
            }
        }
        Ok(())
    }

    /// Restores simultaneous weighted overlay mode.
    pub fn show_overlay(&self, scene: &mut Scene) {
        for &representation in &self.representations {
            scene.show(representation);
        }
    }
}

/// Caller-density representation associated with one weighted ensemble.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProbabilityCloudView {
    /// Source ensemble whose populations produced the caller grid.
    pub ensemble: EnsembleHandle,
    /// Caller-supplied weighted occupancy/density.
    pub volume: VolumeHandle,
    /// Editable direct-volume representation.
    pub representation: RepresentationHandle,
}

/// Semantic ensemble compositions over core scene primitives.
pub trait EnsembleScene {
    /// Draws all members with population-driven opacity and a solid dominant
    /// member.
    ///
    /// # Errors
    ///
    /// Returns a typed error for stale ensemble members or invalid style.
    fn overlay_ensemble(
        &mut self,
        ensemble: EnsembleHandle,
        style: EnsembleStyle,
    ) -> Result<EnsembleView, CoreError>;

    /// Presents a caller-computed weighted occupancy grid as the ensemble's
    /// probability cloud. The renderer never derives or fabricates density.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an absent ensemble or volume.
    fn probability_cloud(
        &mut self,
        ensemble: EnsembleHandle,
        volume: VolumeHandle,
    ) -> Result<ProbabilityCloudView, CoreError>;
}

impl EnsembleScene for Scene {
    fn overlay_ensemble(
        &mut self,
        ensemble: EnsembleHandle,
        style: EnsembleStyle,
    ) -> Result<EnsembleView, CoreError> {
        validate_style(style)?;
        let source = self
            .ensemble(ensemble)
            .cloned()
            .ok_or(CoreError::StaleHandle)?;
        let dominant = source.dominant_index();
        let dominant_weight = source.weights()[dominant];
        let mut selections = Vec::with_capacity(source.members().len());
        let mut representations = Vec::with_capacity(source.members().len());
        for (index, (&member, &weight)) in source.members().iter().zip(source.weights()).enumerate()
        {
            let selection = self.add_structure_selection(member, AtomSelection::All)?;
            let representation = self.represent(selection, style.representation)?;
            let Some(view) = self.representation_mut(representation) else {
                return Err(CoreError::StaleHandle);
            };
            view.color =
                pdviewx_core::ColorScheme::Uniform(MEMBER_COLORS[index % MEMBER_COLORS.len()]);
            view.material = Material::default();
            view.material.opacity =
                member_opacity(index == dominant, weight, dominant_weight, style);
            view.order = u16::try_from(index).map_or(u16::MAX, |order| order);
            selections.push(selection);
            representations.push(representation);
        }
        Ok(EnsembleView {
            ensemble,
            selections,
            representations,
            dominant,
        })
    }

    fn probability_cloud(
        &mut self,
        ensemble: EnsembleHandle,
        volume: VolumeHandle,
    ) -> Result<ProbabilityCloudView, CoreError> {
        if self.ensemble(ensemble).is_none() || self.volume(volume).is_none() {
            return Err(CoreError::StaleHandle);
        }
        let representation = self.represent(volume, pdviewx_core::Representation::volume())?;
        Ok(ProbabilityCloudView {
            ensemble,
            volume,
            representation,
        })
    }
}

fn validate_style(style: EnsembleStyle) -> Result<(), CoreError> {
    if !style.alternate_opacity.is_finite()
        || !style.minimum_opacity.is_finite()
        || !(0.0..=1.0).contains(&style.minimum_opacity)
        || !(style.minimum_opacity..=1.0).contains(&style.alternate_opacity)
    {
        return Err(CoreError::InvalidEnsemble {
            reason: "ensemble opacity bounds must be finite and ordered within zero and one",
        });
    }
    Ok(())
}

fn member_opacity(dominant: bool, weight: f32, dominant_weight: f32, style: EnsembleStyle) -> f32 {
    if dominant {
        1.0
    } else {
        (weight / dominant_weight * style.alternate_opacity)
            .clamp(style.minimum_opacity, style.alternate_opacity)
    }
}
