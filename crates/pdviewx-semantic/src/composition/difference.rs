//! Caller-correspondence differential rendering.

use pdviewx_core::{
    AtomProperty, AtomPropertyHandle, AtomPropertyMeaning, AtomSelection, ColorScheme, CoreError,
    Material, PropertyAppearance, PropertyAppearanceSample, RepresentationHandle,
    RepresentationKind, ScalarFieldSemantics, Scene, SelectionHandle, StructureHandle,
};
use pdviewx_math::Vec3;
use std::sync::Arc;

#[cfg(test)]
#[path = "difference_tests.rs"]
mod tests;

/// One caller-supplied atom-row correspondence between two placed structures.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AtomCorrespondence {
    /// Atom row in the first structure.
    pub left: u32,
    /// Corresponding atom row in the second structure.
    pub right: u32,
}

/// Declarative changed-versus-context presentation.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DifferenceStyle {
    /// Displacements at or below this value remain demoted context.
    pub tolerance_angstrom: f32,
    /// Opacity of unchanged and unmatched context.
    pub context_opacity: f32,
    /// Geometry used for both aligned states.
    pub representation: RepresentationKind,
}

impl Default for DifferenceStyle {
    fn default() -> Self {
        Self {
            tolerance_angstrom: 0.5,
            context_opacity: 0.16,
            representation: RepresentationKind::Cartoon,
        }
    }
}

/// Editable result of differential scene composition.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DifferenceView {
    /// Per-atom displacement columns for left and right structures.
    pub properties: [AtomPropertyHandle; 2],
    /// Structure-scoped source selections.
    pub selections: [SelectionHandle; 2],
    /// Independently editable representations for both states.
    pub representations: [RepresentationHandle; 2],
    /// Largest paired displacement in Ångström.
    pub maximum_displacement: f32,
}

/// Differential composition over caller-supplied atom correspondence.
pub trait DifferenceScene {
    /// Computes paired world-space displacement and composes two reversible
    /// property-driven views. Alignment and correspondence remain upstream.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed, repeated or out-of-range pairs.
    fn render_difference(
        &mut self,
        structures: [StructureHandle; 2],
        correspondence: &[AtomCorrespondence],
        provenance: impl Into<Arc<str>>,
        style: DifferenceStyle,
    ) -> Result<DifferenceView, CoreError>;
}

impl DifferenceScene for Scene {
    fn render_difference(
        &mut self,
        structures: [StructureHandle; 2],
        correspondence: &[AtomCorrespondence],
        provenance: impl Into<Arc<str>>,
        style: DifferenceStyle,
    ) -> Result<DifferenceView, CoreError> {
        validate_style(style)?;
        let provenance = provenance.into();
        if provenance.trim().is_empty() || correspondence.is_empty() {
            return Err(invalid(
                "difference correspondence and provenance must be non-empty",
            ));
        }
        let ([left_values, right_values], maximum) =
            displacement_columns(self, structures, correspondence)?;
        let high = maximum.max(style.tolerance_angstrom + 1.0e-3);
        let semantics = ScalarFieldSemantics::quantity(
            Arc::from("paired atomic displacement"),
            Arc::from("A"),
            provenance,
        )?;
        let left_property = AtomProperty::new(
            structures[0],
            "paired displacement",
            Arc::from(left_values),
            AtomPropertyMeaning::Generic,
            semantics.clone(),
        )?;
        let right_property = AtomProperty::new(
            structures[1],
            "paired displacement",
            Arc::from(right_values),
            AtomPropertyMeaning::Generic,
            semantics,
        )?;
        let handles = [
            self.add_atom_property(left_property)?,
            self.add_atom_property(right_property)?,
        ];
        let left = compose_side(self, structures[0], handles[0], style, high, 0)?;
        let right = compose_side(self, structures[1], handles[1], style, high, 1)?;
        Ok(DifferenceView {
            properties: handles,
            selections: [left.0, right.0],
            representations: [left.1, right.1],
            maximum_displacement: maximum,
        })
    }
}

fn compose_side(
    scene: &mut Scene,
    structure: StructureHandle,
    property: AtomPropertyHandle,
    style: DifferenceStyle,
    high: f32,
    order: u16,
) -> Result<(SelectionHandle, RepresentationHandle), CoreError> {
    let selection = scene.add_structure_selection(structure, AtomSelection::All)?;
    let representation = scene.represent(selection, style.representation)?;
    let color = scene
        .atom_property(property)
        .map(|value| ColorScheme::property(property, value))
        .ok_or(CoreError::StaleHandle)?;
    let appearance = PropertyAppearance::new(
        property,
        [style.tolerance_angstrom, high],
        [style.context_opacity, 1.0],
        [1.4, 0.0],
        PropertyAppearanceSample {
            opacity: style.context_opacity,
            softness_pixels: 1.4,
        },
    )?;
    let view = scene
        .representation_mut(representation)
        .ok_or(CoreError::StaleHandle)?;
    view.color = color;
    view.appearance = Some(appearance);
    view.material = if style.representation == RepresentationKind::Cartoon {
        Material::anisotropic_ribbon(0.35)
    } else {
        Material::default()
    };
    view.order = order;
    Ok((selection, representation))
}

fn displacement_columns(
    scene: &Scene,
    structures: [StructureHandle; 2],
    correspondence: &[AtomCorrespondence],
) -> Result<([Vec<f32>; 2], f32), CoreError> {
    let left = scene
        .structure(structures[0])
        .ok_or(CoreError::StaleHandle)?;
    let right = scene
        .structure(structures[1])
        .ok_or(CoreError::StaleHandle)?;
    let mut values = [
        vec![f32::NAN; left.atoms.len() as usize],
        vec![f32::NAN; right.atoms.len() as usize],
    ];
    let mut maximum = 0.0f32;
    for pair in correspondence {
        let left_index = pair.left as usize;
        let right_index = pair.right as usize;
        if left_index >= values[0].len() || right_index >= values[1].len() {
            return Err(invalid(
                "difference correspondence atom row is out of range",
            ));
        }
        if values[0][left_index].is_finite() || values[1][right_index].is_finite() {
            return Err(invalid(
                "difference correspondence atom rows must be unique",
            ));
        }
        let left_local = Vec3::from_array(left.atoms.coords().slice()[left_index]);
        let right_local = Vec3::from_array(right.atoms.coords().slice()[right_index]);
        let distance = left
            .model_to_world
            .transform_point3(left_local)
            .distance(right.model_to_world.transform_point3(right_local));
        values[0][left_index] = distance;
        values[1][right_index] = distance;
        maximum = maximum.max(distance);
    }
    Ok((values, maximum))
}

fn validate_style(style: DifferenceStyle) -> Result<(), CoreError> {
    if !style.tolerance_angstrom.is_finite()
        || style.tolerance_angstrom < 0.0
        || !style.context_opacity.is_finite()
        || !(0.0..1.0).contains(&style.context_opacity)
    {
        return Err(invalid(
            "difference tolerance and context opacity are invalid",
        ));
    }
    Ok(())
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidDifference { reason }
}
