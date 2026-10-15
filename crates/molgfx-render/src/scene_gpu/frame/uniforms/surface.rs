//! Molecular field support and bounded sampling geometry.
use crate::scene_gpu::detail::GridLimits;
use crate::scene_gpu::grid_fit::{GridFit, fit_grid};
use molgfx_core::{Representation, SurfaceKind};
use molgfx_math::{Aabb, Vec3};
pub(super) struct SurfaceSampling {
    pub(super) surface: [f32; 4],
    pub(super) minimum: Vec3,
    pub(super) fit: GridFit,
}
pub(super) fn sampling(
    representation: &Representation,
    bounds: Aabb,
    limits: GridLimits,
) -> SurfaceSampling {
    let gaussian = representation.params.surface_kind == SurfaceKind::Gaussian;
    let sigma = representation.params.gaussian_sigma;
    let isolevel = if gaussian {
        if representation.params.isolevel > 0.0 {
            representation.params.isolevel
        } else {
            0.5
        }
    } else {
        representation.params.isolevel
    };
    let probe = if representation.params.surface_kind == SurfaceKind::VanDerWaals {
        0.0
    } else if gaussian {
        sigma * representation.params.gaussian_cutoff
    } else {
        representation.params.probe_radius.max(0.0)
    };
    let minimum = bounds.min - Vec3::splat(probe);
    let maximum = bounds.max + Vec3::splat(probe);
    let extent = maximum - minimum;
    let GridFit {
        cell,
        dimensions,
        memory_limited,
    } = fit_grid(
        extent,
        GridLimits {
            spacing: match representation.params.surface_spacing {
                Some(spacing) => spacing,
                None => limits.spacing,
            },
            ..limits
        },
    );
    SurfaceSampling {
        surface: [
            probe,
            isolevel,
            if gaussian { sigma } else { 0.02 },
            if gaussian {
                representation.params.gaussian_cutoff
            } else {
                0.02
            },
        ],
        minimum,
        fit: GridFit {
            cell,
            dimensions,
            memory_limited,
        },
    }
}
