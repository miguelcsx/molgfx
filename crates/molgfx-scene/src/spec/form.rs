//! Lowering of each molecular form into its native geometry controls.

use crate::representation::form::RepresentationFormSpec;

pub(super) fn apply_form(
    native: molgfx_core::RepresentationConfig,
    form: &RepresentationFormSpec,
) -> Result<molgfx_core::RepresentationConfig, crate::Error> {
    match form {
        RepresentationFormSpec::Polyhedra { cutoff } => Ok(native.distance_cutoff(*cutoff)),
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
        RepresentationFormSpec::DynamicBonds {
            cutoff,
            bond_radius,
        } => Ok(native.distance_cutoff(*cutoff).bond_radius(*bond_radius)),
        RepresentationFormSpec::Bonds { bond_radius } => Ok(native.bond_radius(*bond_radius)),
        RepresentationFormSpec::Points { size } => Ok(native.point_size(*size)),
        RepresentationFormSpec::Dots { size, samples } => {
            Ok(native.point_size(*size).point_samples(*samples))
        }
        RepresentationFormSpec::Solvent {
            probe_radius,
            size,
            samples,
        } => Ok(native
            .probe_radius(*probe_radius)
            .point_size(*size)
            .point_samples(*samples)),
        RepresentationFormSpec::QuickSurf {
            radius,
            sigma,
            isolevel,
            grid_spacing,
            cutoff,
        } => Ok(native
            .surface(
                molgfx_core::SurfaceKind::Gaussian,
                molgfx_core::SurfaceStyle::Solid,
            )
            .radius_scale(*radius)
            .gaussian_sigma(*sigma)
            .gaussian_cutoff(*cutoff)
            .surface_spacing(*grid_spacing)
            .isolevel(*isolevel)),
        RepresentationFormSpec::MolecularSurface {
            wire,
            width,
            controls,
            ..
        } => Ok(native
            .surface(
                molgfx_core::SurfaceKind::SolventExcluded,
                if *wire {
                    molgfx_core::SurfaceStyle::Mesh
                } else {
                    molgfx_core::SurfaceStyle::Solid
                },
            )
            .line_width(*width)
            .probe_radius(controls.probe_radius)),
        RepresentationFormSpec::Surface {
            surface,
            style,
            probe_radius,
            isolevel,
            blob_spread,
        } => Ok(apply_surface(
            native,
            *surface,
            *style,
            [*probe_radius, *isolevel, *blob_spread],
        )),
        form => apply_curve_form(native, form),
    }
}

fn apply_curve_form(
    native: molgfx_core::RepresentationConfig,
    form: &RepresentationFormSpec,
) -> Result<molgfx_core::RepresentationConfig, crate::Error> {
    match form {
        RepresentationFormSpec::Ribbons {
            width,
            edge_radius,
            gaps,
        } => Ok(native
            .ribbon_width(*width)
            .bond_radius(*edge_radius)
            .gaps(*gaps)),
        RepresentationFormSpec::NewRibbons {
            width,
            aspect_ratio,
            profile,
            gaps,
        } => native
            .ribbon_width(*width)
            .cartoon_profiles(*profile, *profile)
            .gaps(*gaps)
            .cartoon_shape(*aspect_ratio, 0.0)
            .map_err(crate::Error::from),
        RepresentationFormSpec::Backbone { width, gaps } => native
            .gaps(*gaps)
            .tube_radius(*width)
            .map_err(crate::Error::from),
        RepresentationFormSpec::Trace { radius, gaps }
        | RepresentationFormSpec::Tube { radius, gaps } => native
            .gaps(*gaps)
            .tube_radius(*radius)
            .map_err(crate::Error::from),
        RepresentationFormSpec::Putty {
            gaps,
            domain_min,
            domain_max,
            radius_min,
            radius_max,
        } => native
            .gaps(*gaps)
            .putty_b_factor([*domain_min, *domain_max], [*radius_min, *radius_max])
            .map_err(crate::Error::from),
        RepresentationFormSpec::Cartoon {
            width,
            helix_radius,
            aspect_ratio,
            arrow_factor,
            direction_wedges,
            gaps,
            helix_profile,
            nucleic_profile,
            ..
        } => apply_cartoon(
            native.cartoon_helix_radius(*helix_radius)?,
            *width,
            [*aspect_ratio, *arrow_factor],
            [*helix_profile, *nucleic_profile],
            *direction_wedges,
            *gaps,
        ),
        RepresentationFormSpec::NucleicAcid { width }
        | RepresentationFormSpec::Glycan { width } => Ok(native.ribbon_width(*width)),
        RepresentationFormSpec::PaperChain {
            height,
            max_ring_size,
        } => Ok(native.ribbon_width(*height).max_ring_size(*max_ring_size)),
        _ => Err(crate::Error::InvalidSpec(
            "form has no polymer geometry controls".to_owned(),
        )),
    }
}

fn apply_cartoon(
    native: molgfx_core::RepresentationConfig,
    width: f32,
    shape: [f32; 2],
    profiles: [molgfx_core::CartoonProfile; 2],
    direction_wedges: bool,
    gaps: molgfx_core::GapStyle,
) -> Result<molgfx_core::RepresentationConfig, crate::Error> {
    native
        .ribbon_width(width)
        .gaps(gaps)
        .direction_wedges(direction_wedges)
        .cartoon_profiles(profiles[0], profiles[1])
        .cartoon_shape(shape[0], shape[1])
        .map_err(crate::Error::from)
}

fn apply_surface(
    native: molgfx_core::RepresentationConfig,
    surface: crate::representation::SurfaceKind,
    style: crate::representation::SurfaceStyle,
    controls: [f32; 3],
) -> molgfx_core::RepresentationConfig {
    native
        .surface(surface.native(), style.native())
        .probe_radius(controls[0])
        .isolevel(controls[1])
        .blob_spread(controls[2])
}
