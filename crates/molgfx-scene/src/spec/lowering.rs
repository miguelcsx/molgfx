//! Inward lowering from semantic representation values to the physical engine.

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
        RepresentationFormSpec::BallAndStick { .. }
        | RepresentationFormSpec::Bases { .. }
        | RepresentationFormSpec::BasePairs { .. } => Native::BallAndStick,
        RepresentationFormSpec::Spacefill { .. } => Native::Spacefill,
        RepresentationFormSpec::Licorice { .. } => Native::Licorice,
        RepresentationFormSpec::Lines { .. } => Native::Lines,
        RepresentationFormSpec::Points { .. } | RepresentationFormSpec::Dots { .. } => {
            Native::Points
        }
        RepresentationFormSpec::Surface { .. } => Native::Surface,
        RepresentationFormSpec::Beads { .. } => Native::Beads,
    }
}

fn apply_form(
    native: molgfx_core::RepresentationConfig,
    form: &RepresentationFormSpec,
) -> Result<molgfx_core::RepresentationConfig, crate::Error> {
    match form {
        RepresentationFormSpec::Backbone { width } => {
            native.tube_radius(*width).map_err(crate::Error::from)
        }
        RepresentationFormSpec::Trace { radius } | RepresentationFormSpec::Tube { radius } => {
            native.tube_radius(*radius).map_err(crate::Error::from)
        }
        RepresentationFormSpec::Putty {
            domain_min,
            domain_max,
            radius_min,
            radius_max,
        } => native
            .putty_b_factor([*domain_min, *domain_max], [*radius_min, *radius_max])
            .map_err(crate::Error::from),
        RepresentationFormSpec::Cartoon {
            width,
            aspect_ratio,
            arrow_factor,
            ..
        } => native
            .ribbon_width(*width)
            .cartoon_shape(*aspect_ratio, *arrow_factor)
            .map_err(crate::Error::from),
        RepresentationFormSpec::NucleicAcid { width }
        | RepresentationFormSpec::Glycan { width } => Ok(native.ribbon_width(*width)),
        RepresentationFormSpec::BallAndStick {
            radius,
            bond_radius,
        }
        | RepresentationFormSpec::Licorice {
            radius,
            bond_radius,
        }
        | RepresentationFormSpec::BasePairs {
            radius,
            bond_radius,
        } => Ok(native.radius_scale(*radius).bond_radius(*bond_radius)),
        RepresentationFormSpec::Spacefill { radius }
        | RepresentationFormSpec::Bases { radius }
        | RepresentationFormSpec::Beads { radius } => Ok(native.radius_scale(*radius)),
        RepresentationFormSpec::Lines { width } => Ok(native.line_width(*width)),
        RepresentationFormSpec::Points { size } | RepresentationFormSpec::Dots { size } => {
            Ok(native.point_size(*size))
        }
        RepresentationFormSpec::Surface {
            surface,
            style,
            probe_radius,
            isolevel,
            blob_spread,
        } => Ok(native
            .surface(surface_kind(*surface), surface_style(*style))
            .probe_radius(*probe_radius)
            .isolevel(*isolevel)
            .blob_spread(*blob_spread)),
    }
}

fn surface_kind(kind: crate::representation::SurfaceKind) -> molgfx_core::SurfaceKind {
    use crate::representation::SurfaceKind;
    match kind {
        SurfaceKind::SolventExcluded => molgfx_core::SurfaceKind::SolventExcluded,
        SurfaceKind::VanDerWaals => molgfx_core::SurfaceKind::VanDerWaals,
        SurfaceKind::SolventAccessible => molgfx_core::SurfaceKind::SolventAccessible,
        SurfaceKind::Gaussian => molgfx_core::SurfaceKind::Gaussian,
    }
}

fn surface_style(style: crate::representation::SurfaceStyle) -> molgfx_core::SurfaceStyle {
    use crate::representation::SurfaceStyle;
    match style {
        SurfaceStyle::Solid => molgfx_core::SurfaceStyle::Solid,
        SurfaceStyle::Contour => molgfx_core::SurfaceStyle::Contour,
        SurfaceStyle::Dots => molgfx_core::SurfaceStyle::Dots,
        SurfaceStyle::FilledContour => molgfx_core::SurfaceStyle::FilledContour,
        SurfaceStyle::Mesh => molgfx_core::SurfaceStyle::Mesh,
        SurfaceStyle::SoftUnion => molgfx_core::SurfaceStyle::SoftUnion,
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
