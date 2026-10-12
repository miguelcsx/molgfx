//! Validation of each form's own geometry controls.

use super::form::RepresentationFormSpec;
use super::{CartoonStyle, GapStyle, SurfaceKind, SurfaceStyle};

impl RepresentationFormSpec {
    pub(crate) fn validate_geometry(&self) -> Result<(), crate::Error> {
        match self {
            RepresentationFormSpec::Cartoon { .. } => validate_cartoon(self)?,
            RepresentationFormSpec::QuickSurf { .. }
            | RepresentationFormSpec::Surface { .. }
            | RepresentationFormSpec::Solvent { .. } => validate_boundary(self)?,
            RepresentationFormSpec::MolecularSurface {
                source,
                controls,
                width,
                ..
            } => {
                controls.validate()?;
                positive("triangle edge width", *width)?;
                if source.content_hash.trim().is_empty() {
                    return Err(crate::Error::InvalidSpec(
                        "analytical surface requires a geometry source identity".to_owned(),
                    ));
                }
            }
            RepresentationFormSpec::Polyhedra { cutoff } => positive("distance cutoff", *cutoff)?,
            RepresentationFormSpec::Ribbons {
                width, edge_radius, ..
            } => validate_ribbon(*width, *edge_radius)?,
            RepresentationFormSpec::NewRibbons {
                width,
                aspect_ratio,
                ..
            } => {
                positive("width", *width)?;
                positive("aspect ratio", *aspect_ratio)?;
            }
            RepresentationFormSpec::Backbone { width, .. }
            | RepresentationFormSpec::Lines { width }
            | RepresentationFormSpec::NucleicAcid { width }
            | RepresentationFormSpec::Glycan { width } => positive("width", *width)?,
            RepresentationFormSpec::PaperChain {
                height,
                max_ring_size,
            } => validate_ring(*height, *max_ring_size)?,
            RepresentationFormSpec::DynamicBonds {
                cutoff,
                bond_radius,
            } => {
                positive("distance cutoff", *cutoff)?;
                positive("bond radius", *bond_radius)?;
            }
            RepresentationFormSpec::Bonds { bond_radius } => positive("bond radius", *bond_radius)?,
            RepresentationFormSpec::Points { size } => {
                positive("point size", *size)?;
            }
            RepresentationFormSpec::Dots { size, samples } => validate_points(*size, *samples)?,
            RepresentationFormSpec::Trace { radius, .. }
            | RepresentationFormSpec::Tube { radius, .. } => {
                nonnegative("tube radius", *radius)?;
            }
            RepresentationFormSpec::Putty {
                domain_min,
                domain_max,
                radius_min,
                radius_max,
                ..
            } => {
                validate_putty(*domain_min, *domain_max, *radius_min, *radius_max)?;
            }
            RepresentationFormSpec::Spacefill { radius }
            | RepresentationFormSpec::Bases { radius }
            | RepresentationFormSpec::Beads { radius } => positive("radius", *radius)?,
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
            } => {
                positive("radius", *radius)?;
                positive("bond radius", *bond_radius)?;
            }
        }
        Ok(())
    }
}

fn positive(name: &str, value: f32) -> Result<(), crate::Error> {
    if value.is_finite() && value > 0.0 {
        return Ok(());
    }
    Err(crate::Error::InvalidSpec(format!(
        "{name} must be finite and positive"
    )))
}

fn nonnegative(name: &str, value: f32) -> Result<(), crate::Error> {
    if value.is_finite() && value >= 0.0 {
        return Ok(());
    }
    Err(crate::Error::InvalidSpec(format!(
        "{name} must be finite and nonnegative"
    )))
}

fn validate_ring(height: f32, maximum: u8) -> Result<(), crate::Error> {
    positive("bipyramid height", height)?;
    if !(3..=12).contains(&maximum) {
        return Err(crate::Error::InvalidSpec(
            "ring size must lie between three and twelve".to_owned(),
        ));
    }
    Ok(())
}

fn validate_points(size: f32, samples: u16) -> Result<(), crate::Error> {
    positive("point size", size)?;
    if !(1..=4096).contains(&samples) {
        return Err(crate::Error::InvalidSpec(
            "point samples must lie between one and 4096".to_owned(),
        ));
    }
    Ok(())
}

fn validate_cartoon(form: &RepresentationFormSpec) -> Result<(), crate::Error> {
    let RepresentationFormSpec::Cartoon {
        width,
        helix_radius,
        aspect_ratio,
        arrow_factor,
        direction_wedges,
        style,
        gaps,
        helix_profile,
        nucleic_profile,
        ..
    } = *form
    else {
        return Err(crate::Error::InvalidSpec(
            "cartoon controls require a cartoon form".to_owned(),
        ));
    };
    if (direction_wedges || gaps == GapStyle::Dashed) && style == CartoonStyle::Glycan {
        return Err(crate::Error::InvalidSpec(
            "direction wedges and dashed gaps require a polymer backbone".to_owned(),
        ));
    }
    positive("width", width)?;
    positive("helix radius", helix_radius)?;
    let defaults = molgfx_core::RepresentationParams::default();
    if style != CartoonStyle::Rocket
        && helix_radius.to_bits() != defaults.cartoon_helix_radius.to_bits()
    {
        return Err(crate::Error::InvalidSpec(
            "helix radius requires fitted circular helices; curved ribbons use their width and profile".to_owned(),
        ));
    }
    if style == CartoonStyle::Rocket
        && (helix_profile != defaults.cartoon_helix_profile
            || nucleic_profile != defaults.cartoon_nucleic_profile)
    {
        return Err(crate::Error::InvalidSpec(
            "fitted circular helices have a fixed round profile and no nucleic backbone; profile controls require NewCartoon".to_owned(),
        ));
    }
    if !(helix_radius * 2.0).is_finite() {
        return Err(crate::Error::InvalidSpec(
            "helix diameter must be finite".to_owned(),
        ));
    }
    positive("aspect ratio", aspect_ratio)?;
    if !arrow_factor.is_finite() || arrow_factor < 0.0 {
        return Err(crate::Error::InvalidSpec(
            "arrow factor must be finite and non-negative".to_owned(),
        ));
    }
    Ok(())
}

fn validate_surface(
    surface: SurfaceKind,
    style: SurfaceStyle,
    probe_radius: f32,
    isolevel: f32,
    blob_spread: f32,
) -> Result<(), crate::Error> {
    if !style.native().supports(surface.native()) {
        return Err(crate::Error::InvalidSpec(
            "soft union requires a van der Waals or solvent-accessible boundary".to_owned(),
        ));
    }
    positive("probe radius", probe_radius)?;
    if !isolevel.is_finite() {
        return Err(crate::Error::InvalidSpec(
            "isolevel must be finite".to_owned(),
        ));
    }
    // A blob spread is a distance; it may be zero to request the
    // exact union, but never negative or non-finite.
    if !blob_spread.is_finite() || blob_spread < 0.0 {
        return Err(crate::Error::InvalidSpec(
            "blob spread must be finite and non-negative".to_owned(),
        ));
    }
    Ok(())
}

fn validate_putty(
    domain_min: f32,
    domain_max: f32,
    radius_min: f32,
    radius_max: f32,
) -> Result<(), crate::Error> {
    for (name, value) in [
        ("putty domain minimum", domain_min),
        ("putty domain maximum", domain_max),
    ] {
        if !value.is_finite() {
            return Err(crate::Error::InvalidSpec(format!("{name} must be finite")));
        }
    }
    if domain_max <= domain_min {
        return Err(crate::Error::InvalidSpec(
            "putty domain must be strictly increasing".to_owned(),
        ));
    }
    positive("putty minimum radius", radius_min)?;
    positive("putty maximum radius", radius_max)?;
    Ok(())
}

fn validate_ribbon(width: f32, edge_radius: f32) -> Result<(), crate::Error> {
    positive("width", width)?;
    if edge_radius.is_finite() && edge_radius >= 0.0 {
        return Ok(());
    }
    Err(crate::Error::InvalidSpec(
        "edge radius must be finite and nonnegative".to_owned(),
    ))
}

fn validate_quick_surf(values: [f32; 5]) -> Result<(), crate::Error> {
    for (name, value) in [
        "radius scale",
        "Gaussian sigma",
        "density threshold",
        "grid spacing",
        "Gaussian support",
    ]
    .into_iter()
    .zip(values)
    {
        positive(name, value)?;
    }
    Ok(())
}

fn validate_boundary(form: &RepresentationFormSpec) -> Result<(), crate::Error> {
    match form {
        RepresentationFormSpec::QuickSurf {
            radius,
            sigma,
            isolevel,
            grid_spacing,
            cutoff,
        } => {
            validate_quick_surf([*radius, *sigma, *isolevel, *grid_spacing, *cutoff])?;
        }
        RepresentationFormSpec::Solvent {
            probe_radius,
            size,
            samples,
        } => {
            positive("probe radius", *probe_radius)?;
            validate_points(*size, *samples)?;
        }
        RepresentationFormSpec::Surface {
            probe_radius,
            isolevel,
            blob_spread,
            surface,
            style,
        } => {
            validate_surface(*surface, *style, *probe_radius, *isolevel, *blob_spread)?;
        }
        _ => {
            return Err(crate::Error::InvalidSpec(
                "boundary controls require a surface form".to_owned(),
            ));
        }
    }
    Ok(())
}
