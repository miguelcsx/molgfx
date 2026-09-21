//! Inward lowering from semantic representation values to the physical engine.

use crate::color::ColorSpec;
use crate::spec::{RepresentationForm, RepresentationSpec};
use crate::visual::{ColorExpr, ScalarExpr, VisualStyle};

impl RepresentationSpec {
    pub(crate) fn native(&self) -> Result<molgfx_core::RepresentationConfig, crate::Error> {
        self.validate()?;
        let kind = native_kind(self);
        let material = molgfx_core::Material {
            opacity: visual_opacity(self.visual.as_ref(), self.opacity),
            ..molgfx_core::Material::default()
        };
        let mut native = molgfx_core::RepresentationConfig::new(kind)
            .color(visual_color(self.visual.as_ref(), &self.color)?)
            .material(material);
        if let Some(visual) = &self.visual {
            native = native.visual(crate::visual_native::lower(visual, &self.parameters)?);
        }
        if let Some(radius) = self.radius {
            native = native.radius_scale(radius);
        }
        if let Some(radius) = self.bond_radius {
            native = native.bond_radius(radius);
        }
        if let Some(width) = self.width {
            native = match self.form {
                RepresentationForm::Points => native.point_size(width),
                RepresentationForm::Lines => native.line_width(width),
                _ => native.ribbon_width(width),
            };
        }
        if let Some(radius) = self.probe_radius {
            native = native.probe_radius(radius);
        }
        if let Some(level) = self.isolevel {
            native = native.isolevel(level);
        }
        if self.form == RepresentationForm::Surface {
            native = native.surface(surface_kind(self), surface_style(self));
        }
        Ok(native)
    }

    pub(crate) fn apply_appearance(
        &self,
        representation: &mut molgfx_core::Representation,
    ) -> Result<(), crate::Error> {
        representation.color = visual_color(self.visual.as_ref(), &self.color)?;
        representation.material.opacity = visual_opacity(self.visual.as_ref(), self.opacity);
        representation.visual = self
            .visual
            .as_ref()
            .map(|visual| crate::visual_native::lower(visual, &self.parameters))
            .transpose()?;
        Ok(())
    }
}

fn native_kind(spec: &RepresentationSpec) -> molgfx_core::RepresentationKind {
    use crate::representation::CartoonStyle;
    use molgfx_core::RepresentationKind as Native;
    match (spec.form, spec.cartoon_style) {
        (RepresentationForm::Cartoon, Some(CartoonStyle::Rocket)) => Native::Rocket,
        (RepresentationForm::Cartoon, Some(CartoonStyle::Glycan))
        | (RepresentationForm::Glycan, _) => Native::Twister,
        (RepresentationForm::Cartoon | RepresentationForm::NucleicAcid, _) => Native::Cartoon,
        (
            RepresentationForm::BallAndStick
            | RepresentationForm::Bases
            | RepresentationForm::BasePairs,
            _,
        ) => Native::BallAndStick,
        (RepresentationForm::Spacefill, _) => Native::Spacefill,
        (RepresentationForm::Licorice, _) => Native::Licorice,
        (RepresentationForm::Lines, _) => Native::Lines,
        (RepresentationForm::Points, _) => Native::Points,
        (RepresentationForm::Surface, _) => Native::Surface,
    }
}

fn surface_kind(spec: &RepresentationSpec) -> molgfx_core::SurfaceKind {
    use crate::representation::SurfaceKind;
    match spec.surface_kind {
        None | Some(SurfaceKind::SolventExcluded) => molgfx_core::SurfaceKind::SolventExcluded,
        Some(SurfaceKind::VanDerWaals) => molgfx_core::SurfaceKind::VanDerWaals,
        Some(SurfaceKind::SolventAccessible) => molgfx_core::SurfaceKind::SolventAccessible,
        Some(SurfaceKind::Gaussian) => molgfx_core::SurfaceKind::Gaussian,
    }
}

fn surface_style(spec: &RepresentationSpec) -> molgfx_core::SurfaceStyle {
    use crate::representation::SurfaceStyle;
    match spec.surface_style {
        None | Some(SurfaceStyle::Solid) => molgfx_core::SurfaceStyle::Solid,
        Some(SurfaceStyle::Contour) => molgfx_core::SurfaceStyle::Contour,
        Some(SurfaceStyle::Dots) => molgfx_core::SurfaceStyle::Dots,
        Some(SurfaceStyle::FilledContour) => molgfx_core::SurfaceStyle::FilledContour,
        Some(SurfaceStyle::Mesh) => molgfx_core::SurfaceStyle::Mesh,
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
) -> Result<molgfx_core::ColorScheme, crate::Error> {
    match visual.map(|style| &style.color) {
        Some(ColorExpr::Constant(color)) => Ok(molgfx_core::ColorScheme::Uniform(color.native())),
        _ => fallback.native(),
    }
}
