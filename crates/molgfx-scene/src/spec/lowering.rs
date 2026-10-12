//! Inward lowering from semantic representation values to the physical engine.

use super::form::apply_form;

use crate::color::ColorSpec;
use crate::representation::form::{RepresentationFormSpec, RepresentationSpec};
use crate::visual::{ColorExpr, ScalarExpr, VisualStyle};
use std::collections::BTreeMap;

/// Everything a representation needs from its scene in order to lower.
///
/// Property bindings and interaction channels are both scene facts, so they
/// travel together rather than as separate arguments threaded through every
/// lowering call.
#[derive(Clone, Copy)]
pub(crate) struct Lowering<'a> {
    pub(crate) properties: &'a BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
    pub(crate) surfaces: Option<(
        &'a crate::overlay::OverlayBindings,
        &'a BTreeMap<crate::StructureId, molgfx_core::MolecularSource>,
    )>,
    pub(crate) channels: &'a [Box<str>],
}

impl RepresentationSpec {
    pub(crate) fn native(
        &self,
        lowering: Lowering<'_>,
    ) -> Result<
        (
            molgfx_core::RepresentationConfig,
            Option<crate::visual::ResolvedVisual>,
        ),
        crate::Error,
    > {
        let appearance = self.prepare_appearance(lowering)?;
        let (color, opacity, visual) = appearance.into_parts();
        let kind = native_kind(self);
        let material = molgfx_core::Material {
            opacity,
            ..molgfx_core::Material::default()
        };
        let mut native = molgfx_core::RepresentationConfig::new(kind)
            .color(color)
            .material(material);
        let resolved = if let Some(prepared) = visual {
            let (style, resolved) = prepared.into_parts();
            native = native.visual(style);
            Some(resolved)
        } else {
            None
        };
        native = apply_form(native, &self.form)?;
        if let RepresentationFormSpec::MolecularSurface { source, .. } = &self.form {
            let (bindings, molecules) = lowering.surfaces.ok_or_else(|| {
                crate::Error::InvalidSpec("analytical surface geometry is unresolved".to_owned())
            })?;
            let structure = self.structure_id().ok_or_else(|| {
                crate::Error::InvalidSpec(
                    "analytical surface has no molecular structure".to_owned(),
                )
            })?;
            let binding = bindings
                .molecular_surface(&source.content_hash, structure)
                .ok_or_else(|| {
                    crate::Error::InvalidSpec(
                        "analytical surface geometry is unresolved".to_owned(),
                    )
                })?;
            let molecule = self
                .structure_id()
                .and_then(|id| molecules.get(&id))
                .ok_or(crate::Error::MissingId)?;
            binding.matches(self, molecule)?;
            native = native.molecular_surface(binding.geometry.clone());
        }
        Ok((native, resolved))
    }

    /// The physical base colour alone, for an edit that changes only colour.
    ///
    /// A constant colour in the visual program still wins over the base
    /// colour, exactly as it does when the whole representation lowers.
    pub(crate) fn native_color(
        &self,
        lowering: Lowering<'_>,
    ) -> Result<molgfx_core::ColorScheme, crate::Error> {
        self.validate_values()?;
        let structure = self.common.structure.ok_or_else(|| {
            crate::Error::InvalidSpec("representation has no structure target".to_owned())
        })?;
        visual_color(
            self.common.visual.as_ref(),
            &self.common.color,
            lowering.properties,
            structure,
        )
    }

    pub(crate) fn prepare_appearance(
        &self,
        lowering: Lowering<'_>,
    ) -> Result<PreparedAppearance, crate::Error> {
        self.validate_values()?;
        let structure = self.common.structure.ok_or_else(|| {
            crate::Error::InvalidSpec("representation has no structure target".to_owned())
        })?;
        let visual = self
            .common
            .visual
            .as_ref()
            .map(|visual| visual.prepare(&self.common.parameters, lowering, structure))
            .transpose()?;
        Ok(PreparedAppearance {
            color: visual_color(
                self.common.visual.as_ref(),
                &self.common.color,
                lowering.properties,
                structure,
            )?,
            opacity: visual_opacity(self.common.visual.as_ref(), self.common.opacity),
            visual,
        })
    }
}

pub(crate) struct PreparedAppearance {
    color: molgfx_core::ColorScheme,
    opacity: f32,
    visual: Option<crate::visual::PreparedVisual>,
}

impl PreparedAppearance {
    pub(crate) fn into_parts(
        self,
    ) -> (
        molgfx_core::ColorScheme,
        f32,
        Option<crate::visual::PreparedVisual>,
    ) {
        (self.color, self.opacity, self.visual)
    }

    pub(crate) fn apply(
        self,
        representation: &mut molgfx_core::Representation,
    ) -> Option<crate::visual::ResolvedVisual> {
        representation.color = self.color;
        representation.material.opacity = self.opacity;
        let resolved = self.visual.map(|prepared| {
            let (visual, resolved) = prepared.into_parts();
            representation.visual = Some(visual);
            resolved
        });
        if resolved.is_none() {
            representation.visual = None;
        }
        resolved
    }
}

fn native_kind(spec: &RepresentationSpec) -> molgfx_core::RepresentationKind {
    use molgfx_core::RepresentationKind as Native;
    match &spec.form {
        RepresentationFormSpec::Backbone { .. } | RepresentationFormSpec::Trace { .. } => {
            Native::Trace
        }
        RepresentationFormSpec::Tube { .. } | RepresentationFormSpec::Putty { .. } => Native::Tube,
        RepresentationFormSpec::Cartoon {
            style: crate::representation::CartoonStyle::Rocket,
            ..
        } => Native::Rocket,
        RepresentationFormSpec::Cartoon {
            style: crate::representation::CartoonStyle::Glycan,
            ..
        }
        | RepresentationFormSpec::Glycan { .. } => Native::Twister,
        RepresentationFormSpec::Cartoon { .. } | RepresentationFormSpec::NucleicAcid { .. } => {
            Native::Cartoon
        }
        RepresentationFormSpec::BallAndStick { .. } => Native::BallAndStick,
        RepresentationFormSpec::Bases { .. } => Native::Bases,
        RepresentationFormSpec::BasePairs { .. } => Native::BasePairs,
        RepresentationFormSpec::Spacefill { .. } => Native::Spacefill,
        RepresentationFormSpec::Licorice { .. } => Native::Licorice,
        RepresentationFormSpec::Lines { .. } => Native::Lines,
        RepresentationFormSpec::Bonds { .. } => Native::Bonds,
        RepresentationFormSpec::DynamicBonds { .. } => Native::DynamicBonds,
        RepresentationFormSpec::Polyhedra { .. } => Native::Polyhedra,
        RepresentationFormSpec::NewRibbons { .. } => Native::NewRibbons,
        RepresentationFormSpec::Ribbons { .. } => Native::Ribbons,
        RepresentationFormSpec::PaperChain { .. } => Native::PaperChain,
        RepresentationFormSpec::Points { .. } => Native::Points,
        RepresentationFormSpec::Dots { .. } => Native::Dots,
        RepresentationFormSpec::Solvent { .. } => Native::Solvent,
        RepresentationFormSpec::Surface { .. } | RepresentationFormSpec::QuickSurf { .. } => {
            Native::Surface
        }
        RepresentationFormSpec::Beads { .. } => Native::Beads,
        RepresentationFormSpec::MolecularSurface { .. } => Native::MolecularSurface,
    }
}

fn visual_opacity(visual: Option<&VisualStyle>, fallback: f32) -> f32 {
    match visual.map(|style| &style.opacity) {
        Some(ScalarExpr::Constant(value)) => *value,
        _ => fallback,
    }
}

fn visual_color(
    visual: Option<&VisualStyle>,
    fallback: &ColorSpec,
    properties: &std::collections::BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
    structure: crate::StructureId,
) -> Result<molgfx_core::ColorScheme, crate::Error> {
    match visual.map(|style| &style.color) {
        Some(ColorExpr::Constant(color)) => Ok(molgfx_core::ColorScheme::Uniform(color.native())),
        _ => fallback.native(properties, structure),
    }
}
