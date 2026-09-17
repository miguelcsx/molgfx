//! Atomic validation shared by every generic composition.

use super::{CompositionError, DifferenceLayer, EnsembleCompositionStyle, EnsembleLayer};
use molgfx_core::{AttributeHandle, CoreError, RowDomain, Scene};
use std::collections::BTreeSet;

pub(super) fn opacity_pair(low: f32, high: f32) -> Result<(), CompositionError> {
    if low.is_finite()
        && high.is_finite()
        && (0.0..=1.0).contains(&low)
        && (low..=1.0).contains(&high)
    {
        Ok(())
    } else {
        Err(CompositionError::Invalid("focus opacity bounds"))
    }
}

pub(super) fn difference(
    scene: &Scene,
    layers: &[DifferenceLayer],
    context_threshold: f32,
    emphasis_threshold: f32,
    context_opacity: f32,
) -> Result<(), CompositionError> {
    if layers.is_empty()
        || !context_threshold.is_finite()
        || !emphasis_threshold.is_finite()
        || context_threshold >= emphasis_threshold
        || !context_opacity.is_finite()
        || !(0.0..=1.0).contains(&context_opacity)
    {
        return Err(CompositionError::Invalid(
            "difference thresholds or opacity",
        ));
    }
    domains(scene, layers.iter().map(|layer| layer.domain))?;
    for layer in layers {
        attribute(scene, layer.domain, layer.delta)?;
    }
    Ok(())
}

pub(super) fn ensemble(
    scene: &Scene,
    layers: &[EnsembleLayer],
    style: EnsembleCompositionStyle,
) -> Result<(), CompositionError> {
    if layers.is_empty()
        || layers
            .iter()
            .any(|layer| !layer.weight.is_finite() || layer.weight < 0.0)
        || layers.iter().all(|layer| layer.weight == 0.0)
        || !style.dominant_opacity.is_finite()
        || !style.alternate_opacity.is_finite()
        || !style.minimum_opacity.is_finite()
        || !(0.0..=1.0).contains(&style.minimum_opacity)
        || !(style.minimum_opacity..=style.dominant_opacity).contains(&style.alternate_opacity)
        || !(style.alternate_opacity..=1.0).contains(&style.dominant_opacity)
    {
        return Err(CompositionError::Invalid(
            "ensemble weights or opacity bounds",
        ));
    }
    domains(scene, layers.iter().map(|layer| layer.domain))
}

pub(super) fn attribute(
    scene: &Scene,
    domain: RowDomain,
    handle: AttributeHandle,
) -> Result<(), CompositionError> {
    if scene.row_count(domain).is_none() {
        return Err(CoreError::StaleHandle.into());
    }
    if scene
        .attribute(handle)
        .is_none_or(|column| column.domain() != domain)
    {
        return Err(CompositionError::Invalid("attribute target mismatch"));
    }
    Ok(())
}

fn domains(
    scene: &Scene,
    domains: impl IntoIterator<Item = RowDomain>,
) -> Result<(), CompositionError> {
    let mut unique = BTreeSet::new();
    for domain in domains {
        if scene.row_count(domain).is_none() {
            return Err(CoreError::StaleHandle.into());
        }
        if !unique.insert(domain) {
            return Err(CompositionError::Invalid(
                "composition domains must be unique",
            ));
        }
    }
    Ok(())
}
