//! Lowering from generic composition metadata to bounded visual programs.

use super::{
    CompositionError, DifferenceCompositionStyle, DifferenceLayer, EnsembleCompositionStyle,
    EnsembleLayer, FocusCompositionStyle, FocusLayer, GenericCompositionScene,
    GenericCompositionView, validation,
};
use pdviewx_core::{RowDomain, Scene, VisualDescriptor, VisualProgramBuilder, VisualStyle};

impl GenericCompositionScene for Scene {
    fn compose_focus(
        &mut self,
        layer: FocusLayer,
        style: FocusCompositionStyle,
    ) -> Result<GenericCompositionView, CompositionError> {
        validation::opacity_pair(style.context_opacity, style.focus_opacity)?;
        validation::attribute(self, layer.domain, layer.emphasis)?;
        let mut builder = VisualProgramBuilder::new();
        let emphasis = builder.scalar_attribute(layer.emphasis)?;
        let zero = builder.scalar(0.0)?;
        let one = builder.scalar(1.0)?;
        let weight = builder.smoothstep(zero, one, emphasis)?;
        let context = builder.scalar(style.context_opacity)?;
        let focus = builder.scalar(style.focus_opacity)?;
        let opacity = builder.mix_scalar(context, focus, weight)?;
        builder.set_opacity(opacity)?;
        attach(self, layer.domain, builder, style.order)?;
        Ok(view([layer.domain], Vec::new()))
    }

    fn compose_difference(
        &mut self,
        layers: &[DifferenceLayer],
        style: DifferenceCompositionStyle,
    ) -> Result<GenericCompositionView, CompositionError> {
        validation::difference(
            self,
            layers,
            style.context_threshold,
            style.emphasis_threshold,
            style.context_opacity,
        )?;
        let mut descriptors = Vec::with_capacity(layers.len());
        for (index, layer) in layers.iter().copied().enumerate() {
            let mut builder = VisualProgramBuilder::new();
            let delta = builder.scalar_attribute(layer.delta)?;
            let color = builder.ramp(delta, style.ramp)?;
            let low = builder.scalar(style.context_threshold)?;
            let high = builder.scalar(style.emphasis_threshold)?;
            let weight = builder.smoothstep(low, high, delta)?;
            let context = builder.scalar(style.context_opacity)?;
            let opaque = builder.scalar(1.0)?;
            let opacity = builder.mix_scalar(context, opaque, weight)?;
            builder.set_base_color(color)?;
            builder.set_opacity(opacity)?;
            descriptors.push((
                layer.domain,
                descriptor(builder, add_order(style.order, index))?,
            ));
        }
        attach_all(self, descriptors)?;
        Ok(view(layers.iter().map(|layer| layer.domain), Vec::new()))
    }

    fn compose_ensemble(
        &mut self,
        layers: &[EnsembleLayer],
        style: EnsembleCompositionStyle,
    ) -> Result<GenericCompositionView, CompositionError> {
        validation::ensemble(self, layers, style)?;
        let total = layers.iter().map(|layer| layer.weight).sum::<f32>();
        let normalized = layers
            .iter()
            .map(|layer| layer.weight / total)
            .collect::<Vec<_>>();
        let dominant = normalized
            .iter()
            .copied()
            .enumerate()
            .max_by(|left, right| {
                left.1
                    .total_cmp(&right.1)
                    .then_with(|| right.0.cmp(&left.0))
            })
            .map_or(0, |(index, _)| index);
        let dominant_weight = normalized[dominant];
        let mut descriptors = Vec::with_capacity(layers.len());
        for (index, layer) in layers.iter().copied().enumerate() {
            let opacity = if index == dominant {
                style.dominant_opacity
            } else {
                (normalized[index] / dominant_weight * style.alternate_opacity)
                    .clamp(style.minimum_opacity, style.alternate_opacity)
            };
            let mut builder = VisualProgramBuilder::new();
            let color = builder.color(layer.color.to_f32())?;
            let opacity = builder.scalar(opacity)?;
            builder.set_base_color(color)?;
            builder.set_opacity(opacity)?;
            descriptors.push((
                layer.domain,
                descriptor(builder, add_order(style.order, index))?,
            ));
        }
        attach_all(self, descriptors)?;
        Ok(view(layers.iter().map(|layer| layer.domain), normalized))
    }
}

fn attach(
    scene: &mut Scene,
    domain: RowDomain,
    builder: VisualProgramBuilder,
    order: i32,
) -> Result<(), CompositionError> {
    scene.set_domain_visual(domain, descriptor(builder, order)?)?;
    Ok(())
}

fn descriptor(
    builder: VisualProgramBuilder,
    order: i32,
) -> Result<VisualDescriptor, CompositionError> {
    Ok(VisualDescriptor::new(VisualStyle::new(builder.finish()?)).with_order(order))
}

fn attach_all(
    scene: &mut Scene,
    descriptors: Vec<(RowDomain, VisualDescriptor)>,
) -> Result<(), CompositionError> {
    for (domain, descriptor) in descriptors {
        scene.set_domain_visual(domain, descriptor)?;
    }
    Ok(())
}

fn add_order(base: i32, index: usize) -> i32 {
    base.saturating_add(i32::try_from(index).map_or(i32::MAX, |value| value))
}

fn view(
    domains: impl IntoIterator<Item = RowDomain>,
    normalized_weights: Vec<f32>,
) -> GenericCompositionView {
    GenericCompositionView {
        domains: domains.into_iter().collect(),
        normalized_weights,
    }
}
