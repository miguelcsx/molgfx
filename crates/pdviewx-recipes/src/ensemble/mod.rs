//! Reusable weighted-ensemble presentation over ordinary scene primitives.

use pdviewx_core::{
    AtomSelection, CoreError, RepresentationHandle, RepresentationKind, Scene, SelectionHandle,
    StructureHandle, VisualProgramBuilder, VisualStyle, VolumeHandle,
};
use pdviewx_math::Rgba8;
use std::sync::Arc;

#[cfg(test)]
#[path = "tests.rs"]
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
    /// Caller-owned structures in member order.
    pub members: Vec<StructureHandle>,
    /// Normalized caller weights in member order.
    pub weights: Vec<f32>,
    /// Caller provenance retained by the composition result.
    pub provenance: Arc<str>,
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
        members: &[StructureHandle],
        weights: &[f32],
        provenance: impl Into<Arc<str>>,
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
        volume: VolumeHandle,
    ) -> Result<ProbabilityCloudView, CoreError>;
}

impl EnsembleScene for Scene {
    fn overlay_ensemble(
        &mut self,
        members: &[StructureHandle],
        weights: &[f32],
        provenance: impl Into<Arc<str>>,
        style: EnsembleStyle,
    ) -> Result<EnsembleView, CoreError> {
        validate_style(style)?;
        let provenance = provenance.into();
        let normalized = validate_members(self, members, weights, &provenance)?;
        let dominant = normalized
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(right.1).then_with(|| right.0.cmp(&left.0)))
            .map_or(0, |(index, _)| index);
        let dominant_weight = normalized[dominant];
        let mut selections = Vec::with_capacity(members.len());
        let mut representations = Vec::with_capacity(members.len());
        for (index, (&member, &weight)) in members.iter().zip(&normalized).enumerate() {
            let selection = self.add_structure_selection(member, AtomSelection::All)?;
            let representation = self.represent(selection, style.representation)?;
            let Some(view) = self.representation_mut(representation) else {
                return Err(CoreError::StaleHandle);
            };
            let mut builder = VisualProgramBuilder::new();
            let color = builder
                .color(MEMBER_COLORS[index % MEMBER_COLORS.len()].to_f32())
                .map_err(CoreError::from)?;
            let opacity = builder
                .scalar(member_opacity(
                    index == dominant,
                    weight,
                    dominant_weight,
                    style,
                ))
                .map_err(CoreError::from)?;
            builder.set_base_color(color).map_err(CoreError::from)?;
            builder.set_opacity(opacity).map_err(CoreError::from)?;
            view.visual = Some(VisualStyle::new(builder.finish().map_err(CoreError::from)?));
            view.order = u16::try_from(index).map_or(u16::MAX, |order| order);
            selections.push(selection);
            representations.push(representation);
        }
        Ok(EnsembleView {
            members: members.to_vec(),
            weights: normalized,
            provenance,
            selections,
            representations,
            dominant,
        })
    }

    fn probability_cloud(
        &mut self,
        volume: VolumeHandle,
    ) -> Result<ProbabilityCloudView, CoreError> {
        if self.volume(volume).is_none() {
            return Err(CoreError::StaleHandle);
        }
        let representation = self.represent(volume, pdviewx_core::Representation::volume())?;
        Ok(ProbabilityCloudView {
            volume,
            representation,
        })
    }
}

fn validate_members(
    scene: &Scene,
    members: &[StructureHandle],
    weights: &[f32],
    provenance: &str,
) -> Result<Vec<f32>, CoreError> {
    if members.is_empty() || members.len() != weights.len() || provenance.trim().is_empty() {
        return Err(CoreError::InvalidEnsemble {
            reason: "members, weights and provenance must be non-empty and aligned",
        });
    }
    if members
        .iter()
        .any(|member| scene.structure(*member).is_none())
    {
        return Err(CoreError::StaleHandle);
    }
    let mut unique = members.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if unique.len() != members.len()
        || weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight < 0.0)
    {
        return Err(CoreError::InvalidEnsemble {
            reason: "members must be unique and weights finite and non-negative",
        });
    }
    let total = weights.iter().sum::<f32>();
    if !total.is_finite() || total <= 0.0 {
        return Err(CoreError::InvalidEnsemble {
            reason: "ensemble weight sum must be positive and finite",
        });
    }
    Ok(weights.iter().map(|weight| *weight / total).collect())
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
